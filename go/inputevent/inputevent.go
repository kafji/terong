package inputevent

type EventCode uint16

const (
	CODE_MOUSE_MOVE EventCode = iota + 1
	CODE_MOUSE_CLICK
	CODE_MOUSE_SCROLL
	CODE_KEYBOARD_KEY_DOWN
	CODE_KEYBOARD_KEY_REPEAT
	CODE_KEYBOARD_KEY_UP
)

type InputEvent struct {
	Code EventCode `json:"code"`
	Data any       `json:"data"`
}

func (e *InputEvent) Fix() {
	switch e.Data.(type) {
	case MouseMove:
		e.Code = CODE_MOUSE_MOVE
	case MouseClick:
		e.Code = CODE_MOUSE_CLICK
	case MouseScroll:
		e.Code = CODE_MOUSE_SCROLL
	}
}

func (e *InputEvent) UnmarshalCBOR([]byte) error {
	return nil
}

// mouse

type MouseMove struct {
	DX int16 `json:"dx"`
	DY int16 `json:"dy"`
}

type MouseClick struct {
	Button MouseButton       `json:"button"`
	Action MouseButtonAction `json:"action"`
}

type MouseScroll struct {
	Direction MouseScrollDirection `json:"direction"`
	Count     uint8                `json:"count"`
}

type MouseButton uint8

const (
	LEFT MouseButton = iota + 1
	RIGHT
	MIDDLE
	MOUSE4
	MOUSE5
)

type MouseButtonAction uint8

const (
	ACTION_DOWN MouseButtonAction = iota + 1
	ACTION_UP
)

type MouseScrollDirection uint8

const (
	SCROLL_UP MouseScrollDirection = iota + 1
	SCROLL_DOWN
)

// keyboard

type KeyDown struct {
	Key KeyCode `json:"key"`
}

type KeyRepeat struct {
	Key KeyCode `json:"key"`
}

type KeyUp struct {
	Key KeyCode `json:"key"`
}

type KeyCode uint16
