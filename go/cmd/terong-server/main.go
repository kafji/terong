//go:build windows

package main

import (
	"context"

	"kafji.net/terong/terong/server"
)

func main() {
	ctx := context.Background()
	server.Start(ctx)
}
