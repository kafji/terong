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

var slog = logging.NewLogger("transport/server")

func Start(ctx context.Context, addr string, events <-chan any) <-chan error {
	done := make(chan error)
	go func() {
		err := run(ctx, addr, events)
		done <- err
	}()
	return done
}

func run(ctx context.Context, addr string, events <-chan any) error {
	slog.Info("listening for connection", "address", addr)
	listener, err := (&net.ListenConfig{}).Listen(ctx, "tcp4", addr)
	if err != nil {
		return errors.Join(errors.New("failed to listen"), err)
	}
	defer listener.Close()

	receptionist := newReceptionist(listener)

	sess := &session{Session: transport.EmptySession()}
	defer sess.Close()

	for {
		select {
		case conn, ok := <-receptionist.conns():
			if !ok {
				return receptionist.error()
			}
			if !sess.Closed() {
				slog.Info("rejecting connection, active session exists", "address", conn.RemoteAddr())
				err := conn.Close()
				if err != nil {
					slog.Warn("failed to close connection", "address", conn.RemoteAddr(), "error", err)
				}
				continue
			}
			sess = &session{Session: transport.NewSession(conn)}
			slog.Info("session established", "address", conn.RemoteAddr())
			runSession(ctx, sess)

		case event := <-events:
			if sess.Closed() {
				continue
			}
			slog.Debug("sending event", "event", event)
			if err := sess.writeEvent(event); err != nil {
				slog.Error("failed to write event", "error", err)
				sess.Close()
			}

		case err := <-sess.done():
			slog.Error("session error", "error", err)
			sess.Close()
		}
	}
}

// receptionist handles incoming connections.
type receptionist struct {
	listener net.Listener

	mu  sync.Mutex
	err error

	conns_ chan net.Conn
}

func newReceptionist(listener net.Listener) *receptionist {
	r := &receptionist{listener: listener, conns_: make(chan net.Conn)}

	go func() {
		defer close(r.conns_)
		for {
			conn, err := r.listener.Accept()
			if err != nil {
				r.mu.Lock()
				defer r.mu.Unlock()
				r.err = fmt.Errorf("failed to accept connection: %v", err)
				return
			}
			slog.Info("connected to client", "address", conn.RemoteAddr())
			r.conns_ <- conn
		}
	}()

	return r
}

func (r *receptionist) conns() <-chan net.Conn {
	return r.conns_
}

func (r *receptionist) error() error {
	r.mu.Lock()
	defer r.mu.Unlock()
	return r.err
}

type session struct {
	*transport.Session
	done_ chan error
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
	return s.WriteFrame(frm)
}

func (s *session) done() <-chan error {
	return s.done_
}

func runSession(ctx context.Context, sess *session) {
	go func() {
		defer close(sess.done_)

		for {
			select {
			case <-ctx.Done():
				sess.done_ <- ctx.Err()
				return

			case <-time.After(transport.PingInterval):
				slog.Debug("sending ping")
				if err := sess.WritePing(); err != nil {
					sess.done_ <- fmt.Errorf("failed to write ping: %v", err)
					return
				}

			case <-sess.PingDeadline():
				if sess.Closed() {
					continue
				}
				sess.done_ <- errors.New("client ping deadline exceeded")
				return

			case frm, ok := <-sess.Inbox():
				if !ok {
					sess.done_ <- sess.InboxErr()
					return
				}
				switch frm.Tag {
				case transport.TagPing:
					slog.Debug("ping received")
					sess.ResetPingDeadline()
				}
			}
		}
	}()
}
