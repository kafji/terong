//go:build linux

package main

import (
	"context"
	"os"

	"kafji.net/terong/logging"
	"kafji.net/terong/terong/client"
)

var slog = logging.NewLogger("terong-client/main")

func main() {
	slog.Info("starting", "GOGC", os.Getenv("GOGC"), "GODEBUG", os.Getenv("GODEBUG"), "GOTRACEBACK", os.Getenv("GOTRACEBACK"))

	ctx := context.Background()

	client.Start(ctx)
}
