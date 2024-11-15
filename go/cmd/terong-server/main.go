//go:build windows

package main

import (
	"context"
	"errors"
	"os"
	"os/signal"
	"runtime/pprof"
	"syscall"

	"kafji.net/terong/logging"
	"kafji.net/terong/terong/server"
)

var slog = logging.NewLogger("terong-server/main")

func main() {
	slog.Info("starting", "GOGC", os.Getenv("GOGC"), "GODEBUG", os.Getenv("GODEBUG"), "GOTRACEBACK", os.Getenv("GOTRACEBACK"))

	ctx := context.Background()

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

	server.Start(ctx)
}
