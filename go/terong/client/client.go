package client

import (
	"context"
	"fmt"
	"log/slog"

	"kafji.net/terong/inputsink"
	"kafji.net/terong/terong/config"
	"kafji.net/terong/transport/client"
)

func Start(ctx context.Context) error {
	cfg, err := config.ReadConfig()
	if err != nil {
		return fmt.Errorf("failed to read config: %v", err)
	}

	cancel, runErr := start(ctx, cfg)
	defer cancel()

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()

		case err := <-runErr:
			if err == context.Canceled {
				continue
			}
			return err

		case msg := <-config.Watch(ctx):
			switch v := msg.(type) {
			case error:
				slog.Warn("config watcher error", "error", v)

			case struct{}:
				slog.Info("config changed")
				cancel()
				cfg, err := config.ReadConfig()
				if err != nil {
					return fmt.Errorf("failed to read config: %v", err)
				}
				cancel, runErr = start(ctx, cfg)
			}
		}
	}
}

func start(ctx context.Context, cfg config.Config) (context.CancelFunc, <-chan error) {
	ctx, cancel := context.WithCancel(ctx)
	errs := make(chan error)
	go func() {
		defer close(errs)
		err := run(ctx, cfg)
		if err != nil {
			errs <- err
		}
	}()
	return cancel, errs
}

func run(ctx context.Context, cfg config.Config) error {
	slog.Info("starting app", "config", cfg)

	transportEvents := make(chan any)
	transportError := make(chan error)
	go func() {
		err := client.Start(ctx, cfg.Client.ServerAddr, transportEvents)
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
			return ctx.Err()

		case err := <-transportError:
			return err

		case err := <-sinkError:
			return err

		case event := <-transportEvents:
			slog.Debug("event", "event", event)
			sinkInputs <- event
		}
	}
}
