//go:build (windows || linux) && amd64

package main

import "kafji.net/terong/transportserver"

func main() {
	transportserver.Start()
}
