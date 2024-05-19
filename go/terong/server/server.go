//go:build windows

package server

import (
	"context"
	"fmt"
	"slices"
	"time"

	"golang.org/x/sys/windows"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/inputsource"
	"kafji.net/terong/logging"
	"kafji.net/terong/terong/config"
	"kafji.net/terong/transport/server"
)

var slog = logging.NewLogger("terong/server")

func Start(ctx context.Context) {
	console, err := disableQuickEdit()
	if err != nil {
		slog.Error("failed to disable quick edit", "error", err)
		return
	}
	defer console.restore()

	cfg, err := config.ReadConfig()
	if err != nil {
		slog.Error("failed to read config file", "error", err)
		return
	}

	watcher := config.Watch(ctx)

restart:
	logging.SetLogLevel(cfg.LogLevel)

	slog.Info("starting server", "config", cfg)
	runCtx, cancelRun := context.WithCancel(ctx)
	go run(runCtx, cfg)
	defer cancelRun()

	var ok bool
	for {
		select {
		case <-ctx.Done():
			slog.Error("context error", "error", err)
			return

		case cfg, ok = <-watcher.Configs():
			if !ok {
				slog.Error("config watcher error", "error", watcher.Err())
				return
			}
			slog.Info("configurations changed", "config", cfg)
			cancelRun()
			goto restart
		}
	}
}

func run(ctx context.Context, cfg config.Config) {
	source := inputsource.Start()
	defer source.Stop()

	events := make(chan inputevent.InputEvent)
	defer close(events)

	transport := server.Start(ctx, fmt.Sprintf(":%d", cfg.Server.Port), events)

	relay := false
	toggledAt := time.Time{}

	buffer := keyBuffer{}

	source.SetEatInput(relay)
	source.SetCaptureMouseMove(relay)

	for {
		select {
		case <-ctx.Done():
			slog.Error("context error", "error", ctx.Err())
			return

		case input, ok := <-source.Inputs():
			if !ok {
				slog.Error("input source stopped", "error", source.Error())
				return
			}
			slog.Debug("input received", "input", input)
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

		case err := <-transport:
			slog.Error("transport error", "error", err)
			return
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

type console struct {
	handle  windows.Handle
	oldMode uint32
}

func disableQuickEdit() (console, error) {
	handle, err := windows.GetStdHandle(windows.STD_INPUT_HANDLE)
	if err != nil {
		return console{}, fmt.Errorf("failed to get handle: %v", err)
	}

	var mode uint32
	err = windows.GetConsoleMode(handle, &mode)
	if err != nil {
		return console{}, fmt.Errorf("failed to get mode: %v", err)
	}

	newMode := mode & ^uint32(windows.ENABLE_QUICK_EDIT_MODE)
	err = windows.SetConsoleMode(handle, newMode)
	if err != nil {
		return console{}, fmt.Errorf("failed to set new mode: %v", err)
	}

	return console{handle: handle, oldMode: mode}, nil
}

func (c console) restore() error {
	err := windows.SetConsoleMode(c.handle, c.oldMode)
	if err != nil {
		return fmt.Errorf("failed to set mode: %v", err)
	}
	return nil
}
