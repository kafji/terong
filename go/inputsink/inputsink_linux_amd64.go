package inputsink

/*
#cgo pkg-config: libevdev
#cgo CFLAGS: -Wall
#include "proxy_linux_amd64.h"
#include <stdlib.h>
#include <string.h>

#cgo noescape write_events
#cgo nocallback write_events
*/
import "C"

import (
	"context"
	"fmt"
	"runtime"
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
		code := mouseButtonToEvKey[b]
		codes[C.EV_KEY] = append(codes[C.EV_KEY], code)
	}

	for _, c := range inputevent.KeyCodes {
		code := keyCodeToEvKey[c]
		codes[C.EV_KEY] = append(codes[C.EV_KEY], code)
	}

	for type_, codes := range codes {
		for _, code := range codes {
			ret := C.libevdev_enable_event_code(dev, type_, code, nil)
			err := evdevError(ret)
			if err != nil {
				return nil, fmt.Errorf("failed to enable event code: %w", err)
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
		return fmt.Errorf("failed to create evdev device: %w", err)
	}
	defer C.libevdev_free(dev)

	var uinput *C.struct_libevdev_uinput
	ret := C.libevdev_uinput_create_from_device(dev, C.LIBEVDEV_UINPUT_OPEN_MANAGED, &uinput)
	if err := evdevError(ret); err != nil {
		return fmt.Errorf("failed to create uinput device: %w", err)
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
		event.code = mouseButtonToEvKey[v.Button]
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
		event.code = keyCodeToEvKey[v.Key]
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

	defer runtime.KeepAlive(events)

	ret := C.write_events(uinput, C.size_t(len(events)), (*C.event_t)(unsafe.Pointer(&events[0])))
	if err := evdevError(ret); err != nil {
		return fmt.Errorf("failed to write event: %w", err)
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

var (
	mouseButtonToEvKey = make([]C.uint, inputevent.MouseButtonMouse5+1)
	keyCodeToEvKey     = make([]C.uint, inputevent.Right+1)
)

func init() {
	mouseButtonToEvKey[inputevent.MouseButtonLeft] = C.BTN_LEFT
	mouseButtonToEvKey[inputevent.MouseButtonRight] = C.BTN_RIGHT
	mouseButtonToEvKey[inputevent.MouseButtonMiddle] = C.BTN_MIDDLE
	mouseButtonToEvKey[inputevent.MouseButtonMouse4] = C.BTN_SIDE
	mouseButtonToEvKey[inputevent.MouseButtonMouse5] = C.BTN_EXTRA

	keyCodeToEvKey[inputevent.Escape] = C.KEY_ESC

	keyCodeToEvKey[inputevent.F1] = C.KEY_F1
	keyCodeToEvKey[inputevent.F2] = C.KEY_F2
	keyCodeToEvKey[inputevent.F3] = C.KEY_F3
	keyCodeToEvKey[inputevent.F4] = C.KEY_F4
	keyCodeToEvKey[inputevent.F5] = C.KEY_F5
	keyCodeToEvKey[inputevent.F6] = C.KEY_F6
	keyCodeToEvKey[inputevent.F7] = C.KEY_F7
	keyCodeToEvKey[inputevent.F8] = C.KEY_F8
	keyCodeToEvKey[inputevent.F9] = C.KEY_F9
	keyCodeToEvKey[inputevent.F10] = C.KEY_F10
	keyCodeToEvKey[inputevent.F11] = C.KEY_F11
	keyCodeToEvKey[inputevent.F12] = C.KEY_F12

	keyCodeToEvKey[inputevent.PrintScreen] = C.KEY_PRINT
	keyCodeToEvKey[inputevent.ScrollLock] = C.KEY_SCROLLLOCK
	keyCodeToEvKey[inputevent.PauseBreak] = C.KEY_PAUSE

	keyCodeToEvKey[inputevent.Grave] = C.KEY_GRAVE

	keyCodeToEvKey[inputevent.D1] = C.KEY_1
	keyCodeToEvKey[inputevent.D2] = C.KEY_2
	keyCodeToEvKey[inputevent.D3] = C.KEY_3
	keyCodeToEvKey[inputevent.D4] = C.KEY_4
	keyCodeToEvKey[inputevent.D5] = C.KEY_5
	keyCodeToEvKey[inputevent.D6] = C.KEY_6
	keyCodeToEvKey[inputevent.D7] = C.KEY_7
	keyCodeToEvKey[inputevent.D8] = C.KEY_8
	keyCodeToEvKey[inputevent.D9] = C.KEY_9
	keyCodeToEvKey[inputevent.D0] = C.KEY_0

	keyCodeToEvKey[inputevent.Minus] = C.KEY_MINUS
	keyCodeToEvKey[inputevent.Equal] = C.KEY_EQUAL

	keyCodeToEvKey[inputevent.A] = C.KEY_A
	keyCodeToEvKey[inputevent.B] = C.KEY_B
	keyCodeToEvKey[inputevent.C] = C.KEY_C
	keyCodeToEvKey[inputevent.D] = C.KEY_D
	keyCodeToEvKey[inputevent.E] = C.KEY_E
	keyCodeToEvKey[inputevent.F] = C.KEY_F
	keyCodeToEvKey[inputevent.G] = C.KEY_G
	keyCodeToEvKey[inputevent.H] = C.KEY_H
	keyCodeToEvKey[inputevent.I] = C.KEY_I
	keyCodeToEvKey[inputevent.J] = C.KEY_J
	keyCodeToEvKey[inputevent.K] = C.KEY_K
	keyCodeToEvKey[inputevent.L] = C.KEY_L
	keyCodeToEvKey[inputevent.M] = C.KEY_M
	keyCodeToEvKey[inputevent.N] = C.KEY_N
	keyCodeToEvKey[inputevent.O] = C.KEY_O
	keyCodeToEvKey[inputevent.P] = C.KEY_P
	keyCodeToEvKey[inputevent.Q] = C.KEY_Q
	keyCodeToEvKey[inputevent.R] = C.KEY_R
	keyCodeToEvKey[inputevent.S] = C.KEY_S
	keyCodeToEvKey[inputevent.T] = C.KEY_T
	keyCodeToEvKey[inputevent.U] = C.KEY_U
	keyCodeToEvKey[inputevent.V] = C.KEY_V
	keyCodeToEvKey[inputevent.W] = C.KEY_W
	keyCodeToEvKey[inputevent.X] = C.KEY_X
	keyCodeToEvKey[inputevent.Y] = C.KEY_Y
	keyCodeToEvKey[inputevent.Z] = C.KEY_Z

	keyCodeToEvKey[inputevent.LeftBrace] = C.KEY_LEFTBRACE
	keyCodeToEvKey[inputevent.RightBrace] = C.KEY_RIGHTBRACE

	keyCodeToEvKey[inputevent.SemiColon] = C.KEY_SEMICOLON
	keyCodeToEvKey[inputevent.Apostrophe] = C.KEY_APOSTROPHE

	keyCodeToEvKey[inputevent.Comma] = C.KEY_COMMA
	keyCodeToEvKey[inputevent.Dot] = C.KEY_DOT
	keyCodeToEvKey[inputevent.Slash] = C.KEY_SLASH

	keyCodeToEvKey[inputevent.Backspace] = C.KEY_BACKSPACE
	keyCodeToEvKey[inputevent.BackSlash] = C.KEY_BACKSLASH
	keyCodeToEvKey[inputevent.Enter] = C.KEY_ENTER

	keyCodeToEvKey[inputevent.Space] = C.KEY_SPACE

	keyCodeToEvKey[inputevent.Tab] = C.KEY_TAB
	keyCodeToEvKey[inputevent.CapsLock] = C.KEY_CAPSLOCK

	keyCodeToEvKey[inputevent.LeftShift] = C.KEY_LEFTSHIFT
	keyCodeToEvKey[inputevent.RightShift] = C.KEY_RIGHTSHIFT

	keyCodeToEvKey[inputevent.LeftCtrl] = C.KEY_LEFTCTRL
	keyCodeToEvKey[inputevent.RightCtrl] = C.KEY_RIGHTCTRL

	keyCodeToEvKey[inputevent.LeftAlt] = C.KEY_LEFTALT
	keyCodeToEvKey[inputevent.RightAlt] = C.KEY_RIGHTALT

	keyCodeToEvKey[inputevent.LeftMeta] = C.KEY_LEFTMETA
	keyCodeToEvKey[inputevent.RightMeta] = C.KEY_RIGHTMETA

	keyCodeToEvKey[inputevent.Insert] = C.KEY_INSERT
	keyCodeToEvKey[inputevent.Delete] = C.KEY_DELETE

	keyCodeToEvKey[inputevent.Home] = C.KEY_HOME
	keyCodeToEvKey[inputevent.End] = C.KEY_END

	keyCodeToEvKey[inputevent.PageUp] = C.KEY_PAGEUP
	keyCodeToEvKey[inputevent.PageDown] = C.KEY_PAGEDOWN

	keyCodeToEvKey[inputevent.Up] = C.KEY_UP
	keyCodeToEvKey[inputevent.Left] = C.KEY_LEFT
	keyCodeToEvKey[inputevent.Down] = C.KEY_DOWN
	keyCodeToEvKey[inputevent.Right] = C.KEY_RIGHT
}
