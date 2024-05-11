package transportserver

import (
	"fmt"
	"net"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/inputsource"
)

func Start() {
	addr := ":7070"
	listener, err := net.Listen("tcp", addr)
	if err != nil {
		panic(err)
	}

	var active net.Conn

	for {
		conn, err := listener.Accept()
		if err != nil {
			panic(err)
		}

		if active != nil {
			conn.Close()
			continue
		}

		active = conn
		go handle(conn)
	}
}

func handle(conn net.Conn) {
	defer conn.Close()

	event := make(chan inputevent.InputEvent, 1024)
	h := inputsource.Start(event)
	defer h.Stop()

loop:
	for {
		select {
		case event := <-event:
			var err error

			event.Fix()

			vBuf, err := cbor.Marshal(&event)
			if err != nil {
				fmt.Println(err)
				break loop
			}

			length := uint16(len(vBuf))
			lBuf := []byte{byte(length >> 8), byte(length)}
			_, err = conn.Write(lBuf)
			if err != nil {
				fmt.Println(err)
				break loop
			}

			_, err = conn.Write(vBuf)
			if err != nil {
				fmt.Println(err)
				break loop
			}
		}
	}
}
