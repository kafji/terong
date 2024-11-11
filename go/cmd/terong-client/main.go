//go:build linux

package main

import (
	"context"
	"errors"
	"os"
	"os/signal"
	"runtime/pprof"
	"syscall"

	"kafji.net/terong/logging"
	"kafji.net/terong/terong/client"
)

var slog = logging.NewLogger("terong-client/main")

func main() {
	slog.Info("starting", "GODEBUG", os.Getenv("GODEBUG"))

	f, err := os.Create("terong-client.prof")
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

	client.Start(ctx)
}
