//go:build windows

package main

import (
	"context"

	"kafji.net/terong/logging"
	"kafji.net/terong/terong/server"
)

func main() {
	logging.Filter = func(namespace string) bool { return namespace != "inputsource" }
	ctx := context.Background()
	server.Start(ctx)
}
