//go:build windows

package main

import (
	"context"
	"os"

	"kafji.net/terong/logging"
	"kafji.net/terong/terong/server"
)

var slog = logging.NewLogger("terong-server/main")

func main() {
	slog.Info("starting", "GOGC", os.Getenv("GOGC"), "GODEBUG", os.Getenv("GODEBUG"), "GOTRACEBACK", os.Getenv("GOTRACEBACK"))

	ctx := context.Background()

	server.Start(ctx)
}
