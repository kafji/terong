package transportclient

import (
	"bufio"
	"fmt"
	"io"
	"net"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/transport"
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

		var frame transport.Frame
		err = cbor.Unmarshal(vBuf, &frame)
		if err != nil {
			panic(err)
		}

		var event inputevent.InputEvent

		switch frame.Code {
		case transport.CODE_MOUSE_MOVE:
			var data inputevent.MouseMove
			err := cbor.Unmarshal(frame.Data, &data)
			if err != nil {
				panic(err)
			}
			event.Data = data

		case transport.CODE_MOUSE_CLICK:
			var data inputevent.MouseClick
			err := cbor.Unmarshal(frame.Data, &data)
			if err != nil {
				panic(err)
			}
			event.Data = data

		case transport.CODE_MOUSE_SCROLL:
			var data inputevent.MouseScroll
			err := cbor.Unmarshal(frame.Data, &data)
			if err != nil {
				panic(err)
			}
			event.Data = data
		}

		fmt.Println(event)
	}
}
