package server

import (
	"context"
	"errors"
	"fmt"
	"net"
	"sync"
	"time"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/logging"
	"kafji.net/terong/transport"
)

var slog = logging.New("transport/server")

type Handle struct {
	mu  sync.Mutex
	err error

	done chan struct{}
}

func Start(ctx context.Context, addr string, events <-chan any) *Handle {
	h := &Handle{}
	go func() {
		err := run(ctx, addr, events)
		h.mu.Lock()
		defer h.mu.Unlock()
		h.err = err
		h.done <- struct{}{}
	}()
	return h
}

func (h *Handle) Done() <-chan struct{} {
	return h.done
}

func (h *Handle) Error() error {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.err
}

func run(ctx context.Context, addr string, events <-chan any) error {
	slog.Info("listening for connection", "address", addr)
	listener, err := (&net.ListenConfig{}).Listen(ctx, "tcp", addr)
	if err != nil {
		return errors.Join(errors.New("failed to listen"), err)
	}
	defer listener.Close()

	conns := make(chan any)
	go receptionist(listener, conns)

	sess := emptySession()

	for {
		select {
		// handle incoming connections
		case conn := <-conns:
			switch v := conn.(type) {
			case error:
				slog.Warn("failed to accept connection", "error", err)
				continue
			case net.Conn:
				if !sess.closed {
					slog.Info("rejecting connection, active session exists", "address", v.RemoteAddr())
					err := v.Close()
					if err != nil {
						slog.Warn("failed to close connection", "address", v.RemoteAddr(), "error", err)
					}
					continue
				}
				slog.Info("establishing session", "address", v.RemoteAddr())
				sess = &session{conn: v}
				sess.start()
			}

		// relay event
		case event := <-events:
			if sess.closed {
				continue
			}
			slog.Debug("sending event", "event", event)
			if err := sess.writeEvent(event); err != nil {
				slog.Error("failed to write event", "error", err)
				sess.close()
			}

		// send ping periodicly
		case <-time.After(transport.PingInterval):
			if sess.closed {
				continue
			}
			slog.Debug("sending ping")
			if err := sess.writePing(); err != nil {
				slog.Error("failed to write ping", "error", err)
				sess.close()
			}

		// ping recv deadline
		case <-time.After(time.Until(sess.pingDeadline).Abs()):
			slog.Warn("ping deadline exceeded")
			sess.close()

		// handle incoming messages
		case m := <-sess.inbox:
			switch v := m.(type) {
			case error:
				slog.Warn("failed to receive message: %v", v)
				sess.close()
			case transport.Frame:
				switch v.Tag {
				case transport.TagPing:
					slog.Debug("ping received")
					sess.pingDeadline = time.Now().Add(transport.PingTimeout)
				}
			}
		}
	}
}

func receptionist(listener net.Listener, conns chan<- any) {
	for {
		slog.Debug("waiting for connection")
		conn, err := listener.Accept()
		if err != nil {
			conns <- fmt.Errorf("failed to accept connection: %v", err)
			continue
		}
		slog.Info("connected to client", "address", conn.RemoteAddr())
		conns <- conn
	}
}

type session struct {
	conn         net.Conn
	started      bool
	closed       bool
	inbox        chan any
	pingDeadline time.Time
}

func emptySession() *session {
	return &session{closed: true, pingDeadline: time.Now().Add(24 * time.Hour * 365)}
}

func (s *session) start() {
	if s.started {
		return
	}
	s.inbox = make(chan any)
	go func() {
		for {
			frm, err := transport.ReadFrame(s.conn)
			if err != nil {
				s.inbox <- err
				break
			}
			s.inbox <- frm
		}
	}()
}

func (s *session) close() {
	if s.closed {
		return
	}
	s.closed = true
	slog.Info("closing connection", "address", s.conn.RemoteAddr())
	err := s.conn.Close()
	if err != nil {
		slog.Warn("failed to close connection", "address", s.conn.RemoteAddr(), "error", err)
	}
}

func (s *session) writeEvent(event any) error {
	value, err := cbor.Marshal(&event)
	if err != nil {
		return errors.Join(errors.New("failed to marshal value"), err)
	}

	lengthInt := len(value)
	if lengthInt > transport.MaxLength {
		return errors.New("length is larger than maximum length")
	}
	length := uint16(lengthInt)

	tag, err := transport.TagFor(event)
	if err != nil {
		return errors.Join(errors.New("failed to get tag"), err)
	}

	frm := transport.Frame{Tag: tag, Length: length, Value: value}
	return s.writeFrame(frm)
}

func (s *session) writePing() error {
	frm := transport.Frame{Tag: transport.TagPing, Length: 0}
	return s.writeFrame(frm)
}

func (s *session) writeFrame(frm transport.Frame) error {
	t := time.Now().Add(100 * time.Millisecond)
	err := s.conn.SetWriteDeadline(t)
	if err != err {
		return fmt.Errorf("failed to set write deadline: %v", err)
	}
	return transport.WriteFrame(s.conn, frm)
}
