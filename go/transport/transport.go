package transport

import (
	"github.com/fxamacker/cbor/v2"
)

type Frame struct {
	Code EventCode       `json:"code"`
	Data cbor.RawMessage `json:"data"`
}

type EventCode uint16

const (
	CODE_MOUSE_MOVE EventCode = iota + 1
	CODE_MOUSE_CLICK
	CODE_MOUSE_SCROLL
	CODE_KEYBOARD_KEY_DOWN
	CODE_KEYBOARD_KEY_REPEAT
	CODE_KEYBOARD_KEY_UP
)
