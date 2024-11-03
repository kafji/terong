//go:build windows

package main

import (
	"context"
	"errors"
	"os"
	"os/signal"
	"runtime/pprof"
	"syscall"

	"kafji.net/terong/terong/server"
)

func main() {
	f, err := os.Create("terong-server.prof")
	if err != nil {
		panic(err)
	}
	defer f.Close()
	err = pprof.StartCPUProfile(f)
	if err != nil {
		panic(err)
	}
	defer pprof.StopCPUProfile()

	ctx, cancel := context.WithCancelCause(context.Background())

	s := make(chan os.Signal, 1)
	signal.Notify(s, syscall.SIGINT)
	go func() {
		<-s
		cancel(errors.New("SIGINT"))
		pprof.StopCPUProfile()
	}()

	server.Start(ctx)
}
