package server

import (
	"context"
	"fmt"
	"log/slog"
	"slices"
	"time"

	"kafji.net/terong/inputevent"
	"kafji.net/terong/inputsource"
	"kafji.net/terong/terong"
	"kafji.net/terong/transport/server"
)

func Start(ctx context.Context, args terong.Args) error {
	cfg, err := terong.ReadConfig(args.ConfigFile)
	if err != nil {
		return err
	}

	sourceInputs := make(chan any, 100)
	sourceHandle := inputsource.Start(sourceInputs)
	defer sourceHandle.Stop()

	transportEvents := make(chan any, 100)
	transportError := make(chan error)
	go func() {
		addr := fmt.Sprintf(":%d", cfg.Port)
		err := server.Start(ctx, addr, transportEvents)
		transportError <- err
	}()

	shouldRelay := false
	toggledAt := time.Time{}

	buffer := keyBuffer{}

	sourceHandle.SetShouldEatInput(shouldRelay)
	sourceHandle.SetCaptureMouseMove(shouldRelay)

loop:
	for {
		select {

		case <-ctx.Done():
			return ctx.Err()

		case err := <-transportError:
			slog.Error("transport error", "error", err)
			break loop

		case input := <-sourceInputs:
			slog.Debug("input event", "input", input)

			if shouldRelay {
				transportEvents <- input
			}

			if v, ok := input.(inputevent.KeyPress); ok {
				buffer.push(v)
			}
			if yes, at := buffer.toggleKeyStrokeExists(toggledAt); yes {
				shouldRelay = !shouldRelay
				toggledAt = at
				sourceHandle.SetShouldEatInput(shouldRelay)
				sourceHandle.SetCaptureMouseMove(shouldRelay)
			}
		}
	}

	return nil
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
