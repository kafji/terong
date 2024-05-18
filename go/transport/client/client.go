package client

import (
	"context"
	"errors"
	"fmt"
	"net"
	"slices"
	"time"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
	"kafji.net/terong/transport"
)

var slog = logging.NewLogger("transport/client")

func Start(ctx context.Context, addr string, events chan<- any) <-chan error {
	done := make(chan error)

	go func() {
		for {
			slog.Info("connecting to server", "address", addr)
			conn, err := net.Dial("tcp", addr)
			if err != nil {
				slog.Error("failed to connect to server", "address", addr)
			} else {
				slog.Info("connected to server", "address", addr)
				sess := transport.NewSession(conn)
				sessErr := runSession(ctx, sess, events)
				if err != <-sessErr {
					slog.Error("session error", "error", err)
				}
				slog.Info("closing session", "address", sess.Conn.RemoteAddr())
				sess.Close()
			}

			slog.Info(fmt.Sprintf("reconnecting to server in %d seconds", transport.ReconnectDelay/time.Second))
			select {
			case <-ctx.Done():
				done <- ctx.Err()
				return
			case <-time.After(transport.ReconnectDelay):
			}
		}
	}()

	return done
}

func runSession(ctx context.Context, sess *transport.Session, events chan<- any) <-chan error {
	done := make(chan error)

	go func() {
		for {
			select {
			case <-ctx.Done():
				done <- ctx.Err()
				return

			case <-time.After(transport.PingInterval):
				slog.Debug("sending ping")
				err := sess.WritePing()
				if err != nil {
					done <- err
					return
				}

			case <-sess.PingDeadline():
				done <- errors.New("server ping deadline exceeded")
				return

			case frm, ok := <-sess.Inbox():
				if !ok {
					done <- sess.InboxErr()
					return
				}

				if slices.Contains(transport.TagEvents(), frm.Tag) {
					event, err := unmarshalEvent(frm)
					if err != nil {
						done <- fmt.Errorf("failed to unmarshal event: %v", err)
						return
					}
					slog.Debug("received event", "event", event)
					events <- event
					continue
				}

				switch frm.Tag {
				case transport.TagPing:
					slog.Debug("received ping")
					sess.ResetPingDeadline()
				default:
					slog.Warn("unexpected tag", "tag", frm.Tag)
				}
			}
		}
	}()

	return done
}

func unmarshalEvent(frm transport.Frame) (any, error) {
	var event any
	var err error

	switch frm.Tag {
	case transport.TagEventMouseMove:
		event, err = unmarshal[inputevent.MouseMove](frm.Value)

	case transport.TagEventMouseClick:
		event, err = unmarshal[inputevent.MouseClick](frm.Value)

	case transport.TagEventMouseScroll:
		event, err = unmarshal[inputevent.MouseScroll](frm.Value)

	case transport.TagEventKeyPress:
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
