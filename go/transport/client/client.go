package client

import (
	"context"
	"errors"
	"fmt"
	"net"
	"time"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
	"kafji.net/terong/transport"
)

var slog = logging.NewLogger("transport/client")

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

func Start(ctx context.Context, addr string) *Handle {
	h := &Handle{inputs: make(chan inputevent.InputEvent)}

	go func() {
		defer close(h.inputs)

		dialer := &net.Dialer{Timeout: transport.ConnectTimeout}

		for {
			slog.Info("connecting to server", "address", addr)
			conn, err := dialer.DialContext(ctx, "tcp4", addr)
			if err != nil {
				slog.Error("failed to connect to server", "address", addr)
			} else {
				slog.Info("connected to server", "address", addr)
				sess := &session{Session: transport.NewSession(conn)}
				runSession(ctx, sess, h.inputs)
				err = <-sess.done
				slog.Error("session error", "error", err)
				switch {
				case errors.Is(err, transport.ErrPingTimedOut):
					sess.Close(err.Error())
				default:
					sess.Close("")
				}
			}

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

func runSession(ctx context.Context, sess *session, inputs chan<- inputevent.InputEvent) {
	go func() {
		defer close(sess.done)

		for {
			select {
			case <-ctx.Done():
				sess.done <- ctx.Err()
				return

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
					sess.ResetRecvPingDeadline()
				default:
					slog.Warn("unexpected tag", "tag", frm.Tag)
				}
			}
		}
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
