package server

import (
	"context"
	"errors"
	"log/slog"
	"net"
	"time"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/transport"
)

func Start(ctx context.Context, addr string, events <-chan any) error {
	slog.Info("listening for connection", "address", addr)
	listener, err := (&net.ListenConfig{}).Listen(ctx, "tcp", addr)
	if err != nil {
		return errors.Join(errors.New("failed to listen"), err)
	}
	defer listener.Close()

	conns := make(chan net.Conn)
	receptionistError := make(chan error)
	go func() {
		err := receptionist(listener, conns)
		receptionistError <- err
	}()

	var sess *session

	for {
		select {

		case err := <-receptionistError:
			return err

		case conn := <-conns:
			if sess != nil && !sess.closed {
				slog.Info("rejecting connection", "address", conn.RemoteAddr())
				err := conn.Close()
				if err != nil {
					slog.Warn("failed to close connection", "address", conn.RemoteAddr(), "error", err)
				}
				continue
			}

			slog.Info("session established", "address", conn.RemoteAddr())
			sess = &session{conn: conn}

		case event := <-events:
			if sess == nil || sess.closed {
				continue
			}

			slog.Debug("sending event", "event", event)
			if err := sess.sendEvent(event); err != nil {
				slog.Error("failed to send event", "error", err)

				slog.Info("closing session", "address", sess.conn.RemoteAddr())
				sess.close()
			}
		}
	}
}

func receptionist(listener net.Listener, conns chan<- net.Conn) error {
	for {
		slog.Info("waiting for connection")
		conn, err := listener.Accept()
		if err != nil {
			return errors.Join(errors.New("failed to accept connection"), err)
		}

		slog.Info("connected to client", "address", conn.RemoteAddr())

		conns <- conn
	}
}

type session struct {
	conn   net.Conn
	closed bool
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

func (s *session) sendEvent(event any) error {
	valueBytes, err := cbor.Marshal(&event)
	if err != nil {
		return errors.Join(errors.New("failed to marshal value"), err)
	}

	lengthInt := len(valueBytes)
	if lengthInt > transport.MaxLength {
		return errors.New("length is larger than maximum length")
	}
	length := uint16(lengthInt)

	tag, err := transport.TagFor(event)
	if err != nil {
		return errors.Join(errors.New("failed to get tag"), err)
	}

	t := time.Now().Add(100 * time.Millisecond)
	err = s.conn.SetDeadline(t)
	if err != err {
		return errors.Join(errors.New("failed to set deadline"), err)
	}

	err = transport.WriteTag(s.conn, tag)
	if err != nil {
		return errors.Join(errors.New("failed to write tag"), err)
	}

	err = transport.WriteLength(s.conn, length)
	if err != nil {
		return errors.Join(errors.New("failed to write length"), err)
	}

	_, err = s.conn.Write(valueBytes)
	if err != nil {
		return errors.Join(errors.New("failed to write value"), err)
	}

	return nil
}
