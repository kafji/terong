//go:build windows

package main

import (
	"context"
	"log"
	"log/slog"

	"kafji.net/terong/console"
	"kafji.net/terong/terong/server"
)

func main() {
	log.SetOutput(console.Writer)
	slog.SetLogLoggerLevel(slog.LevelDebug)

	ctx := context.Background()
	err := server.Start(ctx)
	if err != nil {
		log.Fatalln(err)
	}
}
