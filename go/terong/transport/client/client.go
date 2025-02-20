package client

import (
	"context"
	"errors"
	"fmt"
	"net"
	"os"
	"time"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
	"kafji.net/terong/terong/transport"
)

var slog = logging.NewLogger("terong/transport/client")

type Handle struct {
	inputs chan inputevent.InputEvent
	err    error
}

func (h *Handle) Inputs() <-chan inputevent.InputEvent {
	return h.inputs
}

func (h *Handle) Err() error {
	return h.err
}

type Config struct {
	Addr              string
	TLSCertPath       string
	TLSKeyPath        string
	ServerTLSCertPath string
}

func Start(ctx context.Context, cfg *Config) *Handle {
	clientCert, err := os.ReadFile(cfg.TLSCertPath)
	if err != nil {
		err := fmt.Errorf("failed to read tls cert file: %v", err)
		panic(err)
	}
	clientKey, err := os.ReadFile(cfg.TLSKeyPath)
	if err != nil {
		err := fmt.Errorf("failed to read tls cert file: %v", err)
		panic(err)
	}
	serverCert, err := os.ReadFile(cfg.ServerTLSCertPath)
	if err != nil {
		err := fmt.Errorf("failed to read server tls cert file: %v", err)
		panic(err)
	}

	h := &Handle{inputs: make(chan inputevent.InputEvent)}

	go func() {
		defer close(h.inputs)

		dialer := transport.CreateTLSDialer(clientCert, clientKey, serverCert)(&net.Dialer{Timeout: transport.ConnectTimeout})

		var sess *session
		defer func() {
			sess.Close()
		}()

		for {
			slog.Info("connecting to server", "address", cfg.Addr)
			conn, err := dialer.DialContext(ctx, "tcp4", cfg.Addr)
			if err != nil {
				slog.Error("failed to connect to server", "address", cfg.Addr)
				goto reconnect
			}

			slog.Info("connected to server", "address", conn.RemoteAddr())
			sess = newSession(ctx, conn)
			slog.Info("session established", "address", conn.RemoteAddr())
			runSession(ctx, sess, h.inputs)
			err = <-sess.done
			slog.Error("session terminated", "error", err)
			sess.Close()

		reconnect:
			slog.Info(fmt.Sprintf("reconnecting to server in %d seconds", transport.ReconnectDelay/time.Second))
			select {
			case <-ctx.Done():
				h.err = ctx.Err()
				return
			case <-time.After(transport.ReconnectDelay):
			}
		}
	}()

	return h
}

type session struct {
	*transport.Session
	done chan error
}

func newSession(ctx context.Context, conn net.Conn) *session {
	return &session{
		Session: transport.NewSession(ctx, conn),
		done:    make(chan error, 1),
	}
}

func runSession(ctx context.Context, sess *session, inputs chan<- inputevent.InputEvent) {
	go func() {
		err := func() error {
			for {
				select {
				case <-ctx.Done():
					return ctx.Err()

				case <-sess.SendPingDeadline():
					slog.Debug("sending ping")
					if err := sess.SendPing(); err != nil {
						return fmt.Errorf("failed to write ping: %v", err)
					}

				case <-sess.RecvPingDeadline():
					return transport.ErrPingTimedOut

				case frm, ok := <-sess.Inbox():
					if !ok {
						return sess.InboxErr()
					}

					switch frm.Tag {
					case transport.TagMouseMove:
						fallthrough
					case transport.TagMouseClick:
						fallthrough
					case transport.TagMouseScroll:
						fallthrough
					case transport.TagKeyPress:
						event, err := unmarshalEvent(frm)
						if err != nil {
							slog.Warn("failed to unmarshal event", "error", err)
						} else {
							slog.Debug("event received", "event", event)
							inputs <- event
						}

					case transport.TagPing:
						slog.Debug("ping received")
						sess.SetRecvPingDeadline()

					default:
						slog.Warn("unexpected tag", "tag", frm.Tag)
					} // switch
				} // select
			} // for
		}()

		sess.done <- err
	}()
}

func unmarshalEvent(frm transport.Frame) (inputevent.InputEvent, error) {
	var event inputevent.InputEvent
	var err error
	switch frm.Tag {
	case transport.TagMouseMove:
		event, err = unmarshal[inputevent.MouseMove](frm.Value)
	case transport.TagMouseClick:
		event, err = unmarshal[inputevent.MouseClick](frm.Value)
	case transport.TagMouseScroll:
		event, err = unmarshal[inputevent.MouseScroll](frm.Value)
	case transport.TagKeyPress:
		event, err = unmarshal[inputevent.KeyPress](frm.Value)
	default:
		return nil, errors.New("unexpected tag")
	}
	return event, err
}

func unmarshal[T any](r cbor.RawMessage) (T, error) {
	var t T
	err := cbor.Unmarshal(r, &t)
	return t, err
}
