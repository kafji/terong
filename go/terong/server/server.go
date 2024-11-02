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
	"kafji.net/terong/terong/transport/server"
)

var slog = logging.NewLogger("terong/server")

func Start(ctx context.Context) {
	err := disableQuickEdit()
	if err != nil {
		slog.Warn("failed to disable quick edit", "error", err)
	}

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
	runDone := run(runCtx, cfg)
	defer cancelRun()

	var ok bool
loop:
	for {
		select {
		case <-ctx.Done():
			slog.Error("context error", "error", context.Cause(ctx))
			break loop

		case err := <-runDone:
			slog.Error("error", "error", err)
			break loop

		case cfg, ok = <-watcher.Configs():
			if !ok {
				slog.Error("config watcher error", "error", watcher.Err())
				break loop
			}
			slog.Info("configurations changed", "config", cfg)
			cancelRun()
			goto restart
		}
	}
}

func run(ctx context.Context, cfg *config.Config) <-chan error {
	done := make(chan error, 1)

	go func() {
		err := func() error {
			source := inputsource.Start()
			defer source.Stop()

			events := make(chan inputevent.InputEvent)

			transportCfg := &server.Config{
				Addr:              fmt.Sprintf(":%d", cfg.Server.Port),
				TLSCertPath:       cfg.Server.TLSCertPath,
				TLSKeyPath:        cfg.Server.TLSKeyPath,
				ClientTLSCertPath: cfg.Server.ClientTLSCertPath,
			}
			transport := server.Start(ctx, transportCfg, events)

			buffer := keyBuffer{}
			relay := false
			toggledAt := time.Time{}

			source.SetCaptureInputs(relay)

			for {
				select {
				case <-ctx.Done():
					return ctx.Err()

				case input, ok := <-source.Inputs():
					if !ok {
						return source.Error()
					}
					slog.Debug("input received", "input", input)
					if relay {
						events <- input
					}
					if v, ok := input.(inputevent.KeyPress); ok {
						buffer.push(v)
						if yes, at := buffer.toggleKeyStrokeExists(toggledAt); yes {
							slog.Debug("toggling relay")
							relay = !relay
							toggledAt = at
							source.SetCaptureInputs(relay)
						}
					}
				case err := <-transport:
					return err
				}
			}
		}()

		done <- err
	}()

	return done
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

func disableQuickEdit() error {
	handle, err := windows.GetStdHandle(windows.STD_INPUT_HANDLE)
	if err != nil {
		return fmt.Errorf("failed to get handle: %v", err)
	}
	defer windows.CloseHandle(handle)

	var mode uint32
	err = windows.GetConsoleMode(handle, &mode)
	if err != nil {
		return fmt.Errorf("failed to get mode: %v", err)
	}

	mode &= ^uint32(windows.ENABLE_QUICK_EDIT_MODE)
	err = windows.SetConsoleMode(handle, mode)
	if err != nil {
		return fmt.Errorf("failed to set mode: %v", err)
	}

	return nil
}
