package transportserver

import (
	"fmt"
	"net"

	"github.com/fxamacker/cbor/v2"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/inputsource"
	"kafji.net/terong/transport"
)

func Start() {
	addr := ":7070"
	listener, err := net.Listen("tcp", addr)
	if err != nil {
		panic(err)
	}

	var sess session

	for {
		conn, err := listener.Accept()
		if err != nil {
			panic(err)
		}

		if sess != (session{}) && !sess.closed {
			conn.Close()
			continue
		}

		sess = session{conn: conn}
		go handle(&sess)
	}
}

type session struct {
	conn   net.Conn
	closed bool
}

func handle(s *session) {
	defer func() {
		s.conn.Close()
		s.closed = true
	}()

	event := make(chan inputevent.InputEvent, 1024)
	h := inputsource.Start(event)
	defer h.Stop()

loop:
	for {
		select {
		case event := <-event:
			var err error

			var frame transport.Frame

			switch event.Data.(type) {
			case inputevent.MouseMove:
				frame.Code = transport.CODE_MOUSE_MOVE
			case inputevent.MouseClick:
				frame.Code = transport.CODE_MOUSE_CLICK
			case inputevent.MouseScroll:
				frame.Code = transport.CODE_MOUSE_SCROLL
			}

			data, err := cbor.Marshal(&event.Data)
			if err != nil {
				fmt.Println(err)
				break loop
			}
			frame.Data = data

			vBuf, err := cbor.Marshal(&frame)
			if err != nil {
				fmt.Println(err)
				break loop
			}

			length := uint16(len(vBuf))
			lBuf := []byte{byte(length >> 8), byte(length)}
			_, err = s.conn.Write(lBuf)
			if err != nil {
				fmt.Println(err)
				break loop
			}

			_, err = s.conn.Write(vBuf)
			if err != nil {
				fmt.Println(err)
				break loop
			}
		}
	}
}
