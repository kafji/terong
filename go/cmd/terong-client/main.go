//go:build linux

package main

import (
	"context"
	"log/slog"

	"kafji.net/terong/terong/client"
)

func main() {
	slog.SetLogLoggerLevel(slog.LevelDebug)

	ctx := context.Background()
	client.Start(ctx)
}
