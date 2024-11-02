package inputsink

/*
#cgo pkg-config: libevdev
#cgo CFLAGS: -Wall -g -O2
#include "proxy_linux_amd64.h"
#include <stdlib.h>
#include <string.h>
*/
import "C"

import (
	"context"
	"fmt"
	"syscall"
	"time"
	"unsafe"

	"golang.org/x/sys/unix"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
)

// https://www.freedesktop.org/software/libevdev/doc/latest/libevdev_8h.html
// https://www.freedesktop.org/software/libevdev/doc/latest/libevdev-uinput_8h.html

var slog = logging.NewLogger("terong/inputsink")

func createEvdevDevice() (*C.struct_libevdev, error) {
	dev := C.libevdev_new()
	ok := false
	defer func() {
		if ok {
			return
		}
		C.libevdev_free(dev)
	}()

	// libevdev_set_name copies the string argument using strdup
	name := C.CString("Terong Virtual Input Device")
	C.libevdev_set_name(dev, name)
	// the string is safe to free here
	C.free(unsafe.Pointer(name))

	C.libevdev_set_id_bustype(dev, C.BUS_VIRTUAL)

	codes := make(map[C.uint][]C.uint)

	codes[C.EV_SYN] = append(codes[C.EV_SYN], C.SYN_REPORT)

	codes[C.EV_REL] = append(codes[C.EV_REL], C.REL_X, C.REL_Y, C.REL_WHEEL)

	for _, b := range inputevent.MouseButtons {
		code := mouseButtonToEvKey(b)
		codes[C.EV_KEY] = append(codes[C.EV_KEY], code)
	}

	for _, c := range inputevent.KeyCodes {
		code := keyCodeToEvKey(c)
		codes[C.EV_KEY] = append(codes[C.EV_KEY], code)
	}

	for type_, codes := range codes {
		for _, code := range codes {
			ret := C.libevdev_enable_event_code(dev, type_, code, nil)
			err := evdevError(ret)
			if err != nil {
				return nil, fmt.Errorf("failed to enable event code: %v", err)
			}
		}
	}

	ok = true
	return dev, nil
}

func Start(ctx context.Context, source <-chan inputevent.InputEvent) <-chan error {
	done := make(chan error, 1)
	go func() {
		err := start(ctx, source)
		done <- err
	}()
	return done
}

func start(ctx context.Context, source <-chan inputevent.InputEvent) error {
	dev, err := createEvdevDevice()
	if err != nil {
		return fmt.Errorf("failed to create evdev device: %v", err)
	}
	defer C.libevdev_free(dev)

	var uinput *C.struct_libevdev_uinput
	ret := C.libevdev_uinput_create_from_device(dev, C.LIBEVDEV_UINPUT_OPEN_MANAGED, &uinput)
	if err := evdevError(ret); err != nil {
		return fmt.Errorf("failed to create uinput device: %v", err)
	}
	defer C.libevdev_uinput_destroy(uinput)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()

		case input := <-source:
			err := writeInput(uinput, input)
			if err != nil {
				return err
			}
		}
	}
}

func writeInput(uinput *C.struct_libevdev_uinput, input inputevent.InputEvent) error {
	t := time.Now()

	events := make([]C.event_t, 0)

	switch v := input.(type) {
	case inputevent.MouseMove:
		events = append(
			events,
			C.event_t{
				_type: C.EV_REL,
				code:  C.REL_X,
				value: C.int(v.DX),
			},
			C.event_t{
				_type: C.EV_REL,
				code:  C.REL_Y,
				value: C.int(-v.DY),
			},
		)

	case inputevent.MouseClick:
		event := C.event_t{_type: C.EV_KEY}
		event.code = mouseButtonToEvKey(v.Button)
		switch v.Action {
		case inputevent.MouseButtonActionDown:
			event.value = 1
		case inputevent.MouseButtonActionUp:
			event.value = 0
		}
		events = append(events, event)

	case inputevent.MouseScroll:
		event := C.event_t{_type: C.EV_REL, code: C.REL_WHEEL}
		switch v.Direction {
		case inputevent.MouseScrollUp:
			event.value = C.int(v.Count)
		case inputevent.MouseScrollDown:
			event.value = -C.int(v.Count)
		}
		events = append(events, event)

	case inputevent.KeyPress:
		event := C.event_t{_type: C.EV_KEY}
		event.code = keyCodeToEvKey(v.Key)
		switch v.Action {
		case inputevent.KeyActionDown:
			event.value = 1
		case inputevent.KeyActionRepeat:
			event.value = 2
		case inputevent.KeyActionUp:
			event.value = 0
		}
		events = append(events, event)
	}

	events = append(events, C.event_t{_type: C.EV_SYN, code: C.SYN_REPORT, value: 0})

	d := time.Since(t)
	slog.Debug("map input values", "duration_ns", d.Nanoseconds())

	t = time.Now()

	ret := C.write_events(uinput, C.size_t(len(events)), &events[0])
	if err := evdevError(ret); err != nil {
		return fmt.Errorf("failed to write event: %v", err)
	}

	d = time.Since(t)
	slog.Debug("write uinput events", "duration_ns", d.Nanoseconds())

	return nil
}

func evdevError(returnValue C.int) error {
	if returnValue > -1 {
		return nil
	}
	errno := -returnValue
	name := unix.ErrnoName(syscall.Errno(errno))
	desc := C.GoString(C.strerror(errno))
	return fmt.Errorf("%s %d %s", name, errno, desc)
}

func mouseButtonToEvKey(button inputevent.MouseButton) C.uint {
	var evKey C.uint
	switch button {
	case inputevent.MouseButtonLeft:
		evKey = C.BTN_LEFT
	case inputevent.MouseButtonRight:
		evKey = C.BTN_RIGHT
	case inputevent.MouseButtonMiddle:
		evKey = C.BTN_MIDDLE
	case inputevent.MouseButtonMouse4:
		evKey = C.BTN_SIDE
	case inputevent.MouseButtonMouse5:
		evKey = C.BTN_EXTRA
	}
	return evKey
}

func keyCodeToEvKey(code inputevent.KeyCode) C.uint {
	var evKey C.uint
	switch code {
	case inputevent.Escape:
		evKey = C.KEY_ESC

	case inputevent.F1:
		evKey = C.KEY_F1
	case inputevent.F2:
		evKey = C.KEY_F2
	case inputevent.F3:
		evKey = C.KEY_F3
	case inputevent.F4:
		evKey = C.KEY_F4
	case inputevent.F5:
		evKey = C.KEY_F5
	case inputevent.F6:
		evKey = C.KEY_F6
	case inputevent.F7:
		evKey = C.KEY_F7
	case inputevent.F8:
		evKey = C.KEY_F8
	case inputevent.F9:
		evKey = C.KEY_F9
	case inputevent.F10:
		evKey = C.KEY_F10
	case inputevent.F11:
		evKey = C.KEY_F11
	case inputevent.F12:
		evKey = C.KEY_F12

	case inputevent.PrintScreen:
		evKey = C.KEY_PRINT
	case inputevent.ScrollLock:
		evKey = C.KEY_SCROLLLOCK
	case inputevent.PauseBreak:
		evKey = C.KEY_PAUSE

	case inputevent.Grave:
		evKey = C.KEY_GRAVE

	case inputevent.D1:
		evKey = C.KEY_1
	case inputevent.D2:
		evKey = C.KEY_2
	case inputevent.D3:
		evKey = C.KEY_3
	case inputevent.D4:
		evKey = C.KEY_4
	case inputevent.D5:
		evKey = C.KEY_5
	case inputevent.D6:
		evKey = C.KEY_6
	case inputevent.D7:
		evKey = C.KEY_7
	case inputevent.D8:
		evKey = C.KEY_8
	case inputevent.D9:
		evKey = C.KEY_9
	case inputevent.D0:
		evKey = C.KEY_0

	case inputevent.Minus:
		evKey = C.KEY_MINUS
	case inputevent.Equal:
		evKey = C.KEY_EQUAL

	case inputevent.A:
		evKey = C.KEY_A
	case inputevent.B:
		evKey = C.KEY_B
	case inputevent.C:
		evKey = C.KEY_C
	case inputevent.D:
		evKey = C.KEY_D
	case inputevent.E:
		evKey = C.KEY_E
	case inputevent.F:
		evKey = C.KEY_F
	case inputevent.G:
		evKey = C.KEY_G
	case inputevent.H:
		evKey = C.KEY_H
	case inputevent.I:
		evKey = C.KEY_I
	case inputevent.J:
		evKey = C.KEY_J
	case inputevent.K:
		evKey = C.KEY_K
	case inputevent.L:
		evKey = C.KEY_L
	case inputevent.M:
		evKey = C.KEY_M
	case inputevent.N:
		evKey = C.KEY_N
	case inputevent.O:
		evKey = C.KEY_O
	case inputevent.P:
		evKey = C.KEY_P
	case inputevent.Q:
		evKey = C.KEY_Q
	case inputevent.R:
		evKey = C.KEY_R
	case inputevent.S:
		evKey = C.KEY_S
	case inputevent.T:
		evKey = C.KEY_T
	case inputevent.U:
		evKey = C.KEY_U
	case inputevent.V:
		evKey = C.KEY_V
	case inputevent.W:
		evKey = C.KEY_W
	case inputevent.X:
		evKey = C.KEY_X
	case inputevent.Y:
		evKey = C.KEY_Y
	case inputevent.Z:
		evKey = C.KEY_Z

	case inputevent.LeftBrace:
		evKey = C.KEY_LEFTBRACE
	case inputevent.RightBrace:
		evKey = C.KEY_RIGHTBRACE

	case inputevent.SemiColon:
		evKey = C.KEY_SEMICOLON
	case inputevent.Apostrophe:
		evKey = C.KEY_APOSTROPHE

	case inputevent.Comma:
		evKey = C.KEY_COMMA
	case inputevent.Dot:
		evKey = C.KEY_DOT
	case inputevent.Slash:
		evKey = C.KEY_SLASH

	case inputevent.Backspace:
		evKey = C.KEY_BACKSPACE
	case inputevent.BackSlash:
		evKey = C.KEY_BACKSLASH
	case inputevent.Enter:
		evKey = C.KEY_ENTER

	case inputevent.Space:
		evKey = C.KEY_SPACE

	case inputevent.Tab:
		evKey = C.KEY_TAB
	case inputevent.CapsLock:
		evKey = C.KEY_CAPSLOCK

	case inputevent.LeftShift:
		evKey = C.KEY_LEFTSHIFT
	case inputevent.RightShift:
		evKey = C.KEY_RIGHTSHIFT

	case inputevent.LeftCtrl:
		evKey = C.KEY_LEFTCTRL
	case inputevent.RightCtrl:
		evKey = C.KEY_RIGHTCTRL

	case inputevent.LeftAlt:
		evKey = C.KEY_LEFTALT
	case inputevent.RightAlt:
		evKey = C.KEY_RIGHTALT

	case inputevent.LeftMeta:
		evKey = C.KEY_LEFTMETA
	case inputevent.RightMeta:
		evKey = C.KEY_RIGHTMETA

	case inputevent.Insert:
		evKey = C.KEY_INSERT
	case inputevent.Delete:
		evKey = C.KEY_DELETE

	case inputevent.Home:
		evKey = C.KEY_HOME
	case inputevent.End:
		evKey = C.KEY_END

	case inputevent.PageUp:
		evKey = C.KEY_PAGEUP
	case inputevent.PageDown:
		evKey = C.KEY_PAGEDOWN

	case inputevent.Up:
		evKey = C.KEY_UP
	case inputevent.Left:
		evKey = C.KEY_LEFT
	case inputevent.Down:
		evKey = C.KEY_DOWN
	case inputevent.Right:
		evKey = C.KEY_RIGHT
	}
	return evKey
}
