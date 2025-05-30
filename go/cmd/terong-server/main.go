//go:build windows

package main

import (
	"context"
	"net/http"
	_ "net/http/pprof"

	"kafji.net/terong/terong/server"
)

func main() {
	ctx := context.Background()

	go func() {
		if err := http.ListenAndServe("127.0.0.1:6666", nil); err != nil {
			panic(err)
		}
	}()

	server.Start(ctx)
}
