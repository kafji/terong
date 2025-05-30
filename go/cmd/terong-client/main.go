//go:build linux

package main

import (
	"context"
	"net/http"
	_ "net/http/pprof"

	"kafji.net/terong/terong/client"
)

func main() {
	ctx := context.Background()

	go func() {
		if err := http.ListenAndServe("127.0.0.1:5555", nil); err != nil {
			panic(err)
		}
	}()

	client.Start(ctx)
}
