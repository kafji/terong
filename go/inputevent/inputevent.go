package inputevent

type InputEvent interface {
	inputEvent()
}

func (MouseMove) inputEvent()   {}
func (MouseClick) inputEvent()  {}
func (MouseScroll) inputEvent() {}
func (KeyPress) inputEvent()    {}

var _ InputEvent = MouseMove{}
var _ InputEvent = MouseClick{}
var _ InputEvent = MouseScroll{}
var _ InputEvent = KeyPress{}

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
	mouseButtonMinorant MouseButton = iota
	MouseButtonLeft
	MouseButtonRight
	MouseButtonMiddle
	MouseButtonMouse4
	MouseButtonMouse5
	mouseButtonMajorant
)

type MouseButtonAction uint8

const (
	MouseButtonActionDown MouseButtonAction = iota + 1
	MouseButtonActionUp
)

type MouseScrollDirection uint8

const (
	MouseScrollUp MouseScrollDirection = iota + 1
	MouseScrollDown
)

// keyboard

type KeyPress struct {
	Key    KeyCode   `json:"key"`
	Action KeyAction `json:"action"`
}

type KeyAction uint8

const (
	KeyActionDown KeyAction = iota + 1
	KeyActionRepeat
	KeyActionUp
)

type KeyCode uint16

const (
	keyCodeMinorant KeyCode = iota

	Escape

	// function keys

	F1
	F2
	F3
	F4
	F5
	F6
	F7
	F8
	F9
	F10
	F11
	F12

	PrintScreen
	ScrollLock
	PauseBreak

	// The tilde key.
	Grave

	// digits

	D1
	D2
	D3
	D4
	D5
	D6
	D7
	D8
	D9
	D0

	Minus
	Equal

	A
	B
	C
	D
	E
	F
	G
	H
	I
	J
	K
	L
	M
	N
	O
	P
	Q
	R
	S
	T
	U
	V
	W
	X
	Y
	Z

	LeftBrace
	RightBrace

	SemiColon
	Apostrophe

	Comma
	Dot
	Slash

	Backspace
	BackSlash
	Enter

	Space

	Tab
	CapsLock

	LeftShift
	RightShift

	LeftCtrl
	RightCtrl

	LeftAlt
	RightAlt

	LeftMeta
	RightMeta

	Insert
	Delete

	Home
	End

	PageUp
	PageDown

	Up
	Left
	Down
	Right

	keyCodeMajorant
)

type Normalizer struct {
	prev InputEvent
}

func (n *Normalizer) Normalize(event InputEvent) InputEvent {
	prev, ok := n.prev.(KeyPress)
	if !ok || prev.Action != KeyActionDown {
		n.prev = event
		return event
	}

	this, ok := event.(KeyPress)
	if !ok || this.Action != KeyActionDown {
		n.prev = event
		return event
	}

	if this.Key != prev.Key {
		n.prev = event
		return event
	}

	return KeyPress{Key: this.Key, Action: KeyActionRepeat}
}

var MouseButtons = []MouseButton{}

var KeyCodes = []KeyCode{}

func init() {
	for x := mouseButtonMinorant + 1; x < mouseButtonMajorant; x++ {
		MouseButtons = append(MouseButtons, x)
	}

	for x := keyCodeMinorant + 1; x < keyCodeMajorant; x++ {
		KeyCodes = append(KeyCodes, x)
	}
}
