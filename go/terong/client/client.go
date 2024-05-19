//go:build linux

package client

import (
	"context"

	"kafji.net/terong/inputevent"
	"kafji.net/terong/inputsink"
	"kafji.net/terong/logging"
	"kafji.net/terong/terong/config"
	"kafji.net/terong/transport/client"
)

var slog = logging.NewLogger("terong/client")

func Start(ctx context.Context) {
	cfg, err := config.ReadConfig()
	if err != nil {
		slog.Error("failed to read config file", "error", err)
		return
	}

	watcher := config.Watch(ctx)

restart:
	logging.SetLogLevel(cfg.LogLevel)

	slog.Info("starting client", "config", cfg)
	runCtx, cancelRun := context.WithCancel(ctx)
	go run(runCtx, cfg)
	defer cancelRun()

	var ok bool
	for {
		select {
		case <-ctx.Done():
			slog.Error("context error", "error", err)
			return

		case cfg, ok = <-watcher.Changes():
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
	slog.Info("starting app", "config", cfg)

	inputs := make(chan inputevent.InputEvent)
	defer close(inputs)

	transport := client.Start(ctx, cfg.Client.ServerAddr)

	sinkInputs := make(chan any)
	sinkError := make(chan error)
	go func() {
		err := inputsink.Start(ctx, sinkInputs)
		sinkError <- err
	}()

	for {
		select {
		case <-ctx.Done():
			slog.Error("context error", "error", ctx.Err())
			return

		case err := <-sinkError:
			slog.Error("sink error", "error", err)
			return

		case input, ok := <-transport.Inputs():
			if !ok {
				slog.Error("transport error", "error", transport.Err())
				return
			}
			slog.Debug("input received", "input", input)
			sinkInputs <- input
		}
	}
}
