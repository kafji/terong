package server

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"errors"
	"fmt"
	"net"
	"os"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
	"kafji.net/terong/transport"
)

var slog = logging.NewLogger("transport/server")

type Config struct {
	Addr              string
	TLSCertPath       string
	TLSKeyPath        string
	ClientTLSCertPath string
}

func newTLSConfig(cfg *Config) (*tls.Config, error) {
	cert, err := os.ReadFile(cfg.TLSCertPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read tls cert: %v", err)
	}

	key, err := os.ReadFile(cfg.TLSKeyPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read tls key: %v", err)
	}

	keyPair, err := tls.X509KeyPair(cert, key)
	if err != nil {
		return nil, fmt.Errorf("failed to parse key pair: %v", err)
	}

	clientCert, err := os.ReadFile(cfg.ClientTLSCertPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read client cert: %v", err)
	}

	pool := x509.NewCertPool()
	pool.AppendCertsFromPEM(clientCert)

	return &tls.Config{
		Certificates: []tls.Certificate{keyPair},
		ClientAuth:   tls.RequireAndVerifyClientCert,
		ClientCAs:    pool,
	}, nil
}

func Start(ctx context.Context, cfg *Config, inputs <-chan inputevent.InputEvent) <-chan error {
	done := make(chan error)
	go func() {
		err := run(ctx, cfg, inputs)
		done <- err
	}()
	return done
}

func run(ctx context.Context, cfg *Config, inputs <-chan inputevent.InputEvent) error {
	tlsCfg, err := newTLSConfig(cfg)
	if err != nil {
		return err
	}

	slog.Info("listening for connection", "address", cfg.Addr)
	listener, err := (&net.ListenConfig{}).Listen(ctx, "tcp4", cfg.Addr)
	if err != nil {
		return fmt.Errorf("failed to listen: %v", err)
	}
	listener = tls.NewListener(listener, tlsCfg)
	defer listener.Close()

	receptionist := newReceptionist(listener)

	sess := &session{Session: transport.EmptySession()}
	defer sess.Close("server shutting down")

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
			sess = newSession(conn)
			slog.Info("session established", "address", conn.RemoteAddr())
			runSession(ctx, sess)

		case input := <-inputs:
			select {
			case sess.inputs <- input:
			default:
			}

		case err := <-sess.done:
			slog.Error("session error", "error", err)
			switch {
			case errors.Is(err, transport.ErrPingTimedOut):
				sess.Close(err.Error())
			default:
				sess.Close("")
			}
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
				r.err = fmt.Errorf("failed to accept connection: %v", err)
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

func newSession(conn net.Conn) *session {
	s := &session{
		Session: transport.NewSession(conn),
		inputs:  make(chan inputevent.InputEvent, 1),
		done:    make(chan error),
	}
	return s
}

func (s *session) writeInput(input inputevent.InputEvent) error {
	value, err := cbor.Marshal(&input)
	if err != nil {
		return fmt.Errorf("failed to marshal value: %v", err)
	}

	lengthInt := len(value)
	if lengthInt > transport.ValueMaxLength {
		return errors.New("length is larger than maximum value length")
	}
	length := uint16(lengthInt)

	tag, err := transport.TagFor(input)
	if err != nil {
		return fmt.Errorf("failed to get tag: %v", err)
	}

	frm := transport.Frame{Tag: tag, Length: length, Value: value}
	return s.WriteFrame(frm)
}

func runSession(ctx context.Context, sess *session) {
	go func() {
		defer close(sess.done)

		for {
			select {
			case <-ctx.Done():
				sess.done <- ctx.Err()
				return

			case input := <-sess.inputs:
				slog.Debug("sending input", "input", input)
				if err := sess.writeInput(input); err != nil {
					sess.done <- fmt.Errorf("failed to write input: %v", err)
					return
				}

			case <-sess.SendPingDeadline():
				slog.Debug("sending ping")
				if err := sess.SendPing(); err != nil {
					sess.done <- fmt.Errorf("failed to write ping: %v", err)
					return
				}

			case <-sess.RecvPingDeadline():
				sess.done <- transport.ErrPingTimedOut
				return

			case frm, ok := <-sess.Inbox():
				if !ok {
					sess.done <- sess.InboxErr()
					return
				}
				switch frm.Tag {
				case transport.TagPing:
					slog.Debug("ping received")
					sess.ResetRecvPingDeadline()
				default:
					slog.Warn("unexpected tag", "tag", frm.Tag)
				}
			}
		}
	}()
}
