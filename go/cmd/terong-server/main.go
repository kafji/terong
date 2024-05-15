package main

import (
	"context"
	"log/slog"

	"kafji.net/terong/terong"
	"kafji.net/terong/terong/server"
)

func main() {
	ctx := context.Background()
	args := terong.ParseArgs()
	err := server.Start(ctx, args)
	slog.Error("server error", "error", err)
}
