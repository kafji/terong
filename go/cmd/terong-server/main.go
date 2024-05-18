package main

import (
	"context"
	"log"
	"log/slog"

	"kafji.net/terong/console"
	"kafji.net/terong/terong"
	"kafji.net/terong/terong/server"
)

func main() {
	log.SetOutput(console.Writer)
	slog.SetLogLoggerLevel(slog.LevelDebug)

	ctx := context.Background()
	args := terong.ParseArgs()
	err := server.Start(ctx, args)
	slog.Error("server error", "error", err)
}
