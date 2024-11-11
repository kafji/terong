//go:build linux

package client

import (
	"context"

	"kafji.net/terong/inputevent"
	"kafji.net/terong/inputsink"
	"kafji.net/terong/logging"
	"kafji.net/terong/terong/config"
	"kafji.net/terong/terong/transport/client"
)

var slog = logging.NewLogger("terong/client")

func Start(ctx context.Context) {
	cfg, err := config.ReadConfig()
	if err != nil {
		slog.Error("failed to read config file", "error", err)
		return
	}

	logging.SetLogLevel(cfg.LogLevel)

	slog.Info("starting client", "config", cfg)
	runDone := run(ctx, cfg)

	for {
		select {
		case <-ctx.Done():
			slog.Error("context error", "error", context.Cause(ctx))
			return

		case err := <-runDone:
			slog.Error("error", "error", err)
			return
		}
	}
}

func run(ctx context.Context, cfg *config.Config) <-chan error {
	done := make(chan error, 1)

	go func() {
		err := func() error {
			inputs := make(chan inputevent.InputEvent)
			defer close(inputs)

			transportCfg := &client.Config{
				Addr:              cfg.Client.ServerAddr,
				TLSCertPath:       cfg.Client.TLSCertPath,
				TLSKeyPath:        cfg.Client.TLSKeyPath,
				ServerTLSCertPath: cfg.Client.ServerTLSCertPath,
			}
			transport := client.Start(ctx, transportCfg)

			sinkDone := inputsink.Start(ctx, inputs)

			for {
				select {
				case <-ctx.Done():
					return ctx.Err()

				case err := <-sinkDone:
					return err

				case input, ok := <-transport.Inputs():
					if !ok {
						return transport.Err()
					}
					slog.Debug("input received", "input", input)
					inputs <- input
				}
			}
		}()

		done <- err
	}()

	return done
}
