package inputevent

type InputEvent struct {
	Data any `json:"data"`
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
