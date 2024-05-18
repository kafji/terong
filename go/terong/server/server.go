//go:build windows

package server

import (
	"context"
	"fmt"
	"slices"
	"time"

	"kafji.net/terong/inputevent"
	"kafji.net/terong/inputsource"
	"kafji.net/terong/logging"
	"kafji.net/terong/terong/config"
	"kafji.net/terong/transport/server"
)

var slog = logging.New("terong/server")

func Start(ctx context.Context) error {
	cfg, err := config.ReadConfig()
	if err != nil {
		return err
	}

	source := inputsource.Start()
	defer source.Stop()

	events := make(chan any)
	transport := server.Start(ctx, fmt.Sprintf(":%d", cfg.Server.Port), events)

	relay := false
	toggledAt := time.Time{}

	buffer := keyBuffer{}

	source.SetEatInput(relay)
	source.SetCaptureMouseMove(relay)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()

		case input, ok := <-source.Inputs():
			if !ok {
				return fmt.Errorf("input source stopped: %v", source.Error())
			}

			slog.Debug("input", "input", input)
			if relay {
				events <- input
			}
			if v, ok := input.(inputevent.KeyPress); ok {
				buffer.push(v)
			}
			if yes, at := buffer.toggleKeyStrokeExists(toggledAt); yes {
				slog.Debug("toggling relay")
				relay = !relay
				toggledAt = at
				source.SetEatInput(relay)
				source.SetCaptureMouseMove(relay)
			}

		case <-transport.Done():
			return transport.Error()
		}
	}
}

type keyBufferEntry struct {
	k inputevent.KeyPress
	t time.Time
}

type keyBuffer struct {
	buf []keyBufferEntry
}

func (b *keyBuffer) push(k inputevent.KeyPress) {
	if k.Action != inputevent.KeyActionDown && k.Action != inputevent.KeyActionUp {
		return
	}
	i, _ := slices.BinarySearchFunc(
		b.buf,
		time.Now().Add(-300*time.Millisecond),
		func(e keyBufferEntry, t2 time.Time) int {
			t1 := e.t
			return int(t1.UnixMilli() - t2.UnixMilli())
		},
	)
	b.buf = append(b.buf[i:], keyBufferEntry{k: k, t: time.Now()})
}

func (b *keyBuffer) toggleKeyStrokeExists(after time.Time) (bool, time.Time) {
	c := 1
	var t time.Time
	for i := len(b.buf) - 1; i >= 0; i-- {
		e := b.buf[i]
		if e.k.Key != inputevent.RightCtrl {
			continue
		}
		if e.t.UnixMilli() <= after.UnixMilli() {
			return false, time.Time{}
		}
		switch {
		case c == 1 && e.k.Action == inputevent.KeyActionUp:
			t = e.t
			fallthrough
		case c%2 != 0 && e.k.Action == inputevent.KeyActionUp:
			c++
		case c%2 == 0 && e.k.Action == inputevent.KeyActionDown:
			c++
		}
		if c/2 == 2 {
			return true, t
		}
	}
	return false, time.Time{}
}
