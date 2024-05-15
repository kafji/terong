package client

import (
	"context"
	"fmt"
	"log/slog"

	"kafji.net/terong/inputsink"
	"kafji.net/terong/terong"
	"kafji.net/terong/transport/client"
)

func Start(ctx context.Context, args terong.Args) error {
	cfg, err := terong.ReadConfig(args.ConfigFile)
	if err != nil {
		return err
	}

	transportEvents := make(chan any)
	transportError := make(chan error)
	go func() {
		addr := fmt.Sprintf(":%d", cfg.Port)
		err := client.Start(ctx, addr, transportEvents)
		transportError <- err
	}()

	sinkInputs := make(chan any)
	sinkHandle := inputsink.Start(sinkInputs)
	defer sinkHandle.Stop()

loop:
	for {
		select {

		case <-ctx.Done():
			return ctx.Err()

		case err := <-transportError:
			slog.Error("transport error", "error", err)
			break loop

		case event := <-transportEvents:
			sinkInputs <- event
		}
	}

	return nil
}
