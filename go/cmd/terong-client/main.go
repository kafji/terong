//go:build linux

package main

import (
	"context"
	"log"
	"log/slog"

	"kafji.net/terong/console"
	"kafji.net/terong/terong/client"
)

func main() {
	log.SetOutput(console.Writer)
	slog.SetLogLoggerLevel(slog.LevelDebug)

	ctx := context.Background()
	client.Start(ctx)
}
