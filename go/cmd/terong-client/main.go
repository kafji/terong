//go:build linux

package main

import (
	"context"

	"kafji.net/terong/terong/client"
)

func main() {
	ctx := context.Background()
	client.Start(ctx)
}
