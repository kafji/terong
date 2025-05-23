//go:build windows

package main

import (
	"context"
	"runtime"

	"kafji.net/terong/terong/server"
)

func main() {
	runtime.GOMAXPROCS(max(runtime.NumCPU()/2, 2))

	ctx := context.Background()
	server.Start(ctx)
}
