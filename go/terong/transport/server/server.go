package server

import (
	"context"
	"errors"
	"fmt"
	"net"
	"os"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
	"kafji.net/terong/terong/transport"
)

var slog = logging.NewLogger("terong/transport/server")

type Config struct {
	Addr              string
	TLSCertPath       string
	TLSKeyPath        string
	ClientTLSCertPath string
}

func Start(ctx context.Context, cfg *Config, inputs <-chan inputevent.InputEvent) <-chan error {
	done := make(chan error, 1)
	go func() {
		err := run(ctx, cfg, inputs)
		done <- err
	}()
	return done
}

func run(ctx context.Context, cfg *Config, inputs <-chan inputevent.InputEvent) error {
	serverCert, err := os.ReadFile(cfg.TLSCertPath)
	if err != nil {
		err := fmt.Errorf("failed to read tls cert file: %w", err)
		panic(err)
	}
	serverKey, err := os.ReadFile(cfg.TLSKeyPath)
	if err != nil {
		err := fmt.Errorf("failed to read tls key file: %w", err)
		panic(err)
	}
	clientCert, err := os.ReadFile(cfg.ClientTLSCertPath)
	if err != nil {
		err := fmt.Errorf("failed to read client cert file: %w", err)
		panic(err)
	}

	slog.Info("listening for connection", "address", cfg.Addr)
	listener, err := (&net.ListenConfig{}).Listen(ctx, "tcp4", cfg.Addr)
	if err != nil {
		return fmt.Errorf("failed to listen: %w", err)
	}
	listener = transport.CreateTLSListener(serverCert, serverKey, clientCert)(listener)
	defer listener.Close()

	receptionist := newReceptionist(listener)

	sess := emptySession()
	defer func() {
		sess.Close()
	}()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()

		case conn, ok := <-receptionist.conns:
			if !ok {
				return receptionist.err
			}
			if !sess.Closed() {
				slog.Info("rejecting connection, active session exists", "address", conn.RemoteAddr())
				err := conn.Close()
				if err != nil {
					slog.Warn("failed to close connection", "address", conn.RemoteAddr(), "error", err)
				}
				continue
			}
			sess = newSession(ctx, conn)
			slog.Info("session established", "address", conn.RemoteAddr())
			runSession(ctx, sess)

		case input := <-inputs:
			select {
			case sess.inputs <- input:
			default:
			}

		case err := <-sess.done:
			slog.Error("session terminated", "error", err)
			sess.Close()
		}
	}
}

// receptionist handles incoming connections.
type receptionist struct {
	listener net.Listener
	conns    chan net.Conn
	err      error
}

func newReceptionist(listener net.Listener) *receptionist {
	r := &receptionist{
		listener: listener,
		conns:    make(chan net.Conn),
	}

	go func() {
		defer close(r.conns)

		for {
			conn, err := r.listener.Accept()
			if err != nil {
				r.err = fmt.Errorf("failed to accept connection: %w", err)
				return
			}
			slog.Info("connected to client", "address", conn.RemoteAddr())
			r.conns <- conn
		}
	}()

	return r
}

type session struct {
	*transport.Session
	inputs chan inputevent.InputEvent
	done   chan error
}

func emptySession() *session {
	return &session{Session: transport.EmptySession()}
}

func newSession(ctx context.Context, conn net.Conn) *session {
	return &session{
		Session: transport.NewSession(ctx, conn),
		inputs:  make(chan inputevent.InputEvent, 1),
		done:    make(chan error, 1),
	}
}

func (s *session) writeInput(input inputevent.InputEvent) error {
	value, err := cbor.Marshal(&input)
	if err != nil {
		return fmt.Errorf("failed to marshal value: %w", err)
	}

	lengthInt := len(value)
	if lengthInt > transport.ValueMaxLength {
		return errors.New("length is larger than maximum value length")
	}
	length := uint16(lengthInt)

	tag, err := transport.TagFor(input)
	if err != nil {
		return fmt.Errorf("failed to get tag: %w", err)
	}

	frm := transport.Frame{Tag: tag, Length: length, Value: value}
	return s.WriteFrame(frm)
}

func runSession(ctx context.Context, sess *session) {
	go func() {
		err := func() error {
			for {
				select {
				case <-ctx.Done():
					return ctx.Err()

				case input := <-sess.inputs:
					slog.Debug("sending input", "input", input)
					if err := sess.writeInput(input); err != nil {
						return fmt.Errorf("failed to write input: %w", err)
					}

				case <-sess.SendPingDeadline():
					slog.Debug("sending ping")
					if err := sess.SendPing(); err != nil {
						return fmt.Errorf("failed to write ping: %w", err)
					}

				case <-sess.RecvPingDeadline():
					return transport.ErrPingTimedOut

				case frm, ok := <-sess.Inbox():
					if !ok {
						return sess.InboxErr()
					}
					switch frm.Tag {
					case transport.TagPing:
						slog.Debug("ping received")
						sess.SetRecvPingDeadline()
					default:
						slog.Warn("unexpected tag", "tag", frm.Tag)
					}
				}
			}
		}()

		sess.done <- err
	}()
}
