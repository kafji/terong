package client

import (
	"bufio"
	"context"
	"errors"
	"io"
	"log/slog"
	"net"
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

		err = runSession(ctx, &session{conn: conn, events: events})
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
	conn   net.Conn
	events chan<- any
}

func runSession(ctx context.Context, sess *session) error {
	defer sess.conn.Close()

	reader := bufio.NewReader(sess.conn)

loop:
	for {
		if err := ctx.Err(); err != nil {
			return err
		}

		t := time.Now().Add(100 * time.Millisecond)
		err := sess.conn.SetDeadline(t)
		if err != nil {
			slog.Error("failed to set deadline", "error", err)
			break
		}

		tag, err := transport.ReadTag(reader)
		if err != nil {
			slog.Error("failed to read tag", "error", err)
			break
		}

		length, err := transport.ReadLength(reader)
		if err != nil {
			slog.Error("failed to read length", "error", err)
			break
		}

		valueBytes := make([]byte, length)
		_, err = io.ReadFull(reader, valueBytes)
		if err != nil {
			slog.Error("failed to read value bytes", "error", err)
			break
		}

		if length > transport.MaxLength {
			slog.Warn("length is larger than maximum length", "length", length, "maximum_length", transport.MaxLength)
			continue loop
		}

		var value any

		switch tag {

		case transport.TagMouseMoveEvent:
			value, err = unmarshal[inputevent.MouseMove](valueBytes)

		case transport.TagMouseClickEvent:
			value, err = unmarshal[inputevent.MouseClick](valueBytes)

		case transport.TagMouseScrollEvent:
			value, err = unmarshal[inputevent.MouseScroll](valueBytes)

		case transport.TagKeyPressEvent:
			value, err = unmarshal[inputevent.KeyPress](valueBytes)

		default:
			slog.Warn("unexpected tag", "tag", tag)
			continue loop
		}

		if err != nil {
			slog.Warn("failed to unmarshal value", "error", err)
			continue loop
		}

		sess.events <- value
	}

	return nil
}

func unmarshal[T any](r cbor.RawMessage) (T, error) {
	var t T
	err := cbor.Unmarshal(r, &t)
	return t, err
}
