package main

import (
	"context"
	"log/slog"

	"kafji.net/terong/terong"
	"kafji.net/terong/terong/client"
)

func main() {
	ctx := context.Background()
	args := terong.ParseArgs()
	err := client.Start(ctx, args)
	slog.Error("client error", "error", err)
}
