//go:build linux

package main

import (
	"context"
	"os"
	"runtime/pprof"

	"kafji.net/terong/terong/client"
)

func main() {
	ctx := context.Background()

	f, err := os.Create("terong-client.prof")
	if err != nil {
		panic(err)
	}
	err = pprof.StartCPUProfile(f)
	if err != nil {
		panic(err)
	}
	defer pprof.StopCPUProfile()

	client.Start(ctx)
}
