package client

import (
	"context"
	"errors"
	"log/slog"
	"net"
	"slices"
	"time"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/transport"
)

func Start(ctx context.Context, addr string, events chan<- any) error {
	for {
		slog.Info("connecting to server", "address", addr)
		conn, err := net.Dial("tcp", addr)
		if err != nil {
			slog.Error("failed to connect to server", "address", addr)
			goto reconnect
		}

		slog.Info("connected to server", "address", addr)
		err = runSession(ctx, &session{Session: transport.NewSession(conn), events: events})
		if err != nil {
			return errors.Join(errors.New("session error"), err)
		}

	reconnect:
		slog.Info("reconnecting to server in 5 seconds")
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(5 * time.Second):
		}
	}
}

type session struct {
	*transport.Session
	events chan<- any
}

func runSession(ctx context.Context, sess *session) error {
	defer sess.Close()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()

		case <-time.After(transport.PingInterval):
			err := sess.WritePing()
			if err != nil {
				return err
			}

		case frm, ok := <-sess.Inbox():
			if !ok {
				return sess.Error()
			}
			if slices.Contains(transport.TagEvents(), frm.Tag) {
				event, err := unmarshalEvent(frm)
				if err != nil {
					return err
				}
				sess.events <- event
				continue
			}
			switch frm.Tag {
			case transport.TagPing:
				sess.ResetPingDeadline()
			default:
				slog.Warn("unexpected tag", "tag", frm.Tag)
			}
		}
	}
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
