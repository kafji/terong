package main

import (
	"context"
	"log"
	"log/slog"

	"kafji.net/terong/console"
	"kafji.net/terong/terong"
	"kafji.net/terong/terong/client"
)

func main() {
	log.SetOutput(console.Writer)

	ctx := context.Background()
	args := terong.ParseArgs()
	err := client.Start(ctx, args)
	slog.Error("client error", "error", err)
}
