//go:build windows

package main

import (
	"context"
	"os"
	"runtime/pprof"

	"kafji.net/terong/terong/server"
)

func main() {
	ctx := context.Background()

	f, err := os.Create("terong-server.prof")
	if err != nil {
		panic(err)
	}
	err = pprof.StartCPUProfile(f)
	if err != nil {
		panic(err)
	}
	defer pprof.StopCPUProfile()

	server.Start(ctx)
}
