//go:build (linux || windows) && amd64

package main

import "kafji.net/terong/transportclient"

func main() {
	transportclient.Start()
}
