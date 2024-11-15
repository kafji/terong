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
	slog.Info("starting", "GOGC", os.Getenv("GOGC"), "GODEBUG", os.Getenv("GODEBUG"), "GOTRACEBACK", os.Getenv("GOTRACEBACK"))

	ctx := context.Background()

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

	ctx, cancel := context.WithCancelCause(ctx)
	defer cancel(nil)

	s := make(chan os.Signal, 1)
	signal.Notify(s, syscall.SIGINT)
	go func() {
		defer pprof.StopCPUProfile()
		defer cancel(errors.New("SIGINT"))
		<-s
		signal.Stop(s)
	}()

	client.Start(ctx)
}
