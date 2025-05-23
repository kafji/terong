//go:build linux

package main

import (
	"context"
	"runtime"

	"kafji.net/terong/terong/client"
)

func main() {
	runtime.GOMAXPROCS(max(runtime.NumCPU()/2, 2))

	ctx := context.Background()
	client.Start(ctx)
}
