package transportclient

import (
	"bufio"
	"fmt"
	"io"
	"net"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
)

func Start() {
	addr := "127.0.0.1:7070"
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		panic(err)
	}

	reader := bufio.NewReader(conn)

	for {
		var err error

		lBuf := make([]byte, 2)
		_, err = io.ReadFull(reader, lBuf)
		if err != nil {
			panic(err)
		}

		length := uint16(0)
		length |= uint16(lBuf[0]) << 8
		length |= uint16(lBuf[1])

		vBuf := make([]byte, length)
		_, err = io.ReadFull(reader, vBuf)
		if err != nil {
			panic(err)
		}

		var event inputevent.InputEvent
		err = cbor.Unmarshal(vBuf, &event)
		if err != nil {
			panic(err)
		}

		fmt.Println(event)
	}
}
