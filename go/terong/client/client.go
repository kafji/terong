//go:build linux

package client

import (
	"context"

	"kafji.net/terong/inputsink"
	"kafji.net/terong/logging"
	"kafji.net/terong/terong/config"
	"kafji.net/terong/transport/client"
)

var slog = logging.NewLogger("terong/client")

func Start(ctx context.Context) {
	watcher := config.Watch(ctx)

restart:
	cfg, err := config.ReadConfig()
	if err != nil {
		slog.Error("failed to read config", "error", err)
		return
	}

	appCtx, cancelApp := context.WithCancel(ctx)
	app := startApp(appCtx, cfg)

	for {
		select {
		case <-ctx.Done():
			slog.Error("cancelled", "error", err)
			return

		case err := <-app.done():
			if err != nil {
				slog.Error("app error", "error", err)
			}
			return

		case _, ok := <-watcher.Changed():
			if !ok {
				slog.Error("watcher error", "error", watcher.Error())
				return
			}
			slog.Info("config changed")
			cancelApp()
			goto restart
		}
	}
}

type app struct {
	cfg   config.Config
	done_ chan error
}

func startApp(ctx context.Context, cfg config.Config) *app {
	a := &app{cfg: cfg, done_: make(chan error)}

	defer close(a.done_)

	go func() {
		slog.Info("starting app", "config", a.cfg)

		transportEvents := make(chan any)
		transportError := make(chan error)
		go func() {
			err := client.Start(ctx, a.cfg.Client.ServerAddr, transportEvents)
			transportError <- err
		}()

		sinkInputs := make(chan any)
		sinkError := make(chan error)
		go func() {
			err := inputsink.Start(ctx, sinkInputs)
			if err != nil {
				sinkError <- err
			}
		}()

		for {
			select {
			case <-ctx.Done():
				a.done_ <- ctx.Err()
				return

			case err := <-transportError:
				a.done_ <- err
				return

			case err := <-sinkError:
				a.done_ <- err
				return

			case event := <-transportEvents:
				slog.Debug("event", "event", event)
				sinkInputs <- event
			}
		}
	}()

	return a
}

func (a *app) done() <-chan error {
	return a.done_
}
