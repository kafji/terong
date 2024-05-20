package inputsource

/*
#cgo CFLAGS: -Wall -g -O2
#include <windows.h>
#include "hook_windows_amd64.h"
*/
import "C"

import (
	"runtime"
	"sync"
	"unsafe"

	"golang.org/x/sys/windows"
	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
)

var slog = logging.NewLogger("inputsource")

type Handle struct {
	mu       sync.Mutex
	threadID C.DWORD
	stopped  bool
	err      error

	inputs        chan inputevent.InputEvent
	captureInputs bool
	screenCenter  point
	cursorPos     *C.POINT
}

func Start() *Handle {
	h := &Handle{inputs: make(chan inputevent.InputEvent, 1_000)}
	h.mu.Lock() // lock 'a
	go func() {
		runtime.LockOSThread()
		h.threadID = C.GetCurrentThreadId()
		h.mu.Unlock() // unlock 'a
		err := run(h)
		runtime.UnlockOSThread()

		h.mu.Lock()
		defer h.mu.Unlock()
		h.stopped = true
		h.err = err
		close(h.inputs)
	}()
	return h
}

func (h *Handle) Inputs() <-chan inputevent.InputEvent {
	return h.inputs
}

func (h *Handle) Error() error {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.err
}

func (h *Handle) Stop() {
	if h.stopped {
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	if h.stopped {
		return
	}
	C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_CONTROL_COMMAND, C.CONTROL_COMMAND_STOP, 0)
}

func (h *Handle) SetCaptureInputs(flag bool) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if flag {
		C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_CAPTURE_INPUTS, C.TRUE, 0)
	} else {
		C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_CAPTURE_INPUTS, C.FALSE, 0)
	}
}

func run(handle *Handle) error {
	var err error

	// https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulehandleexw
	var moduleHandle C.HMODULE
	ret := C.GetModuleHandleExW(0, nil, &moduleHandle)
	if ret == 0 {
		return windows.GetLastError()
	}

	// https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc
	mouseHook := C.SetWindowsHookExW(C.WH_MOUSE_LL, (*[0]byte)(C.mouse_hook_proc), moduleHandle, 0)
	if mouseHook == nil {
		return windows.GetLastError()
	}
	defer C.UnhookWindowsHookEx(mouseHook)

	// https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
	keyboardHook := C.SetWindowsHookExW(C.WH_KEYBOARD_LL, (*[0]byte)(C.keyboard_hook_proc), moduleHandle, 0)
	if keyboardHook == nil {
		return windows.GetLastError()
	}
	defer C.UnhookWindowsHookEx(keyboardHook)

	handle.screenCenter, err = screenCenter()
	if err != nil {
		return err
	}

	normalizer := inputevent.Normalizer{}

	// https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues
	for {
		// Achtung!
		//
		// This message loop must never be blocked.
		//
		// When this loop get blocked the user's input will get incredibly choppy.
		//
		// Past cases where this message loop get blocked were:
		//
		// 1. Sending to unbuffered channel.
		// 2. Writing to stdio + QuickEdit.

		if err := windows.GetLastError(); err != nil {
			return err
		}

		if handle.captureInputs {
			// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setcursorpos
			ret := C.SetCursorPos(C.int(handle.screenCenter.x), C.int(handle.screenCenter.y))
			if ret == 0 {
				return windows.GetLastError()
			}
		}

		// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessagew
		var msg C.MSG
		ret := C.get_message(&msg)
		if ret == 0 {
			return nil
		}
		if ret < 0 {
			return windows.GetLastError()
		}

		slog.Debug("message received", "message", msg, "mouse_hook_proc_worst_ms", C.get_mouse_hook_proc_worst(), "keyboard_hook_proc_worst_ms", C.get_keyboard_hook_proc_worst())

		switch msg.message {
		case C.MESSAGE_CODE_HOOK_EVENT:
			hookEvent := C.get_hook_event()
			var input inputevent.InputEvent
			switch msg.wParam {
			case C.WH_MOUSE_LL:
				switch hookEvent.code {
				case C.WM_MOUSEMOVE:
					if !handle.captureInputs {
						continue
					}
					data := (*C.mouse_move_t)(unsafe.Pointer(&hookEvent.data))
					dx := data.x - C.LONG(handle.screenCenter.x)
					dy := -(data.y - C.LONG(handle.screenCenter.y))
					input = inputevent.MouseMove{DX: int16(dx), DY: int16(dy)}

				case C.WM_LBUTTONDOWN:
					input = inputevent.MouseClick{Button: inputevent.MouseButtonLeft, Action: inputevent.MouseButtonActionDown}

				case C.WM_LBUTTONUP:
					input = inputevent.MouseClick{Button: inputevent.MouseButtonLeft, Action: inputevent.MouseButtonActionUp}

				case C.WM_RBUTTONDOWN:
					input = inputevent.MouseClick{Button: inputevent.MouseButtonRight, Action: inputevent.MouseButtonActionDown}

				case C.WM_RBUTTONUP:
					input = inputevent.MouseClick{Button: inputevent.MouseButtonRight, Action: inputevent.MouseButtonActionUp}

				case C.WM_MBUTTONDOWN:
					input = inputevent.MouseClick{Button: inputevent.MouseButtonMiddle, Action: inputevent.MouseButtonActionDown}

				case C.WM_MBUTTONUP:
					input = inputevent.MouseClick{Button: inputevent.MouseButtonMiddle, Action: inputevent.MouseButtonActionUp}

				case C.WM_XBUTTONDOWN:
					data := (*C.mouse_click_t)(unsafe.Pointer(&hookEvent.data))
					button := xbuttonToMouseButton(data.button)
					if button != 0 {
						input = inputevent.MouseClick{Button: button, Action: inputevent.MouseButtonActionDown}
					}

				case C.WM_XBUTTONUP:
					data := (*C.mouse_click_t)(unsafe.Pointer(&hookEvent.data))
					button := xbuttonToMouseButton(data.button)
					if button != 0 {
						input = inputevent.MouseClick{Button: button, Action: inputevent.MouseButtonActionUp}
					}

				case C.WM_MOUSEWHEEL:
					data := (*C.mouse_scroll_t)(unsafe.Pointer(&hookEvent.data))
					count := int(data.distance) / int(C.WHEEL_DELTA)
					switch {
					case count > 0:
						input = inputevent.MouseScroll{Count: uint8(count), Direction: inputevent.MouseScrollUp}
					case count < 0:
						input = inputevent.MouseScroll{Count: uint8(-count), Direction: inputevent.MouseScrollDown}
					case count == 0:
					}
				}

			case C.WH_KEYBOARD_LL:
				switch hookEvent.code {
				case C.WM_KEYDOWN:
					fallthrough
				case C.WM_SYSKEYDOWN:
					data := (*C.key_press_t)(unsafe.Pointer(&hookEvent.data))
					key := keyCodeToVirtualKey(data.virtual_key)
					input = inputevent.KeyPress{Key: key, Action: inputevent.KeyActionDown}

				case C.WM_KEYUP:
					fallthrough
				case C.WM_SYSKEYUP:
					data := (*C.key_press_t)(unsafe.Pointer(&hookEvent.data))
					key := keyCodeToVirtualKey(data.virtual_key)
					input = inputevent.KeyPress{Key: key, Action: inputevent.KeyActionUp}
				}
			}

			slog.Debug("sending input", "input", input)
			if input != nil {
				input = normalizer.Normalize(input)
				select {
				case handle.inputs <- input:
				default:
					slog.Warn("dropping input, channel was blocked", "input", input)
				}
			}

		case C.MESSAGE_CODE_CONTROL_COMMAND:
			switch msg.wParam {
			case C.CONTROL_COMMAND_STOP:
				handle.mu.Lock()
				handle.stopped = true
				handle.mu.Unlock()
				return nil
			}

		case C.MESSAGE_CODE_SET_CAPTURE_INPUTS:
			switch C.BOOL(msg.wParam) {
			case C.TRUE:
				handle.captureInputs = true
			case C.FALSE:
				handle.captureInputs = false
			}
			C.set_eat_input(C.BOOL(msg.wParam))
			if handle.captureInputs {
				handle.cursorPos = &C.POINT{}
				ret := C.GetCursorPos(handle.cursorPos)
				if ret == 0 {
					return windows.GetLastError()
				}
			} else if handle.cursorPos != nil {
				ret := C.SetCursorPos(C.int(handle.cursorPos.x), C.int(handle.cursorPos.y))
				if ret == 0 {
					return windows.GetLastError()
				}
			}
		} // switch
	} // for
}

type point struct {
	x uint16
	y uint16
}

func screenSize() (point, error) {
	rect := C.RECT{}
	// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow
	ret := C.SystemParametersInfoW(C.SPI_GETWORKAREA, 0, C.PVOID(&rect), 0)
	if ret == 0 {
		return point{}, windows.GetLastError()

	}
	return point{x: uint16(rect.right - rect.left), y: uint16(rect.bottom - rect.top)}, nil
}

func screenCenter() (point, error) {
	screen, err := screenSize()
	if err != nil {
		return point{}, err
	}
	return point{x: screen.x / 2, y: screen.y / 2}, nil
}

func xbuttonToMouseButton(xbutton C.WORD) inputevent.MouseButton {
	var button inputevent.MouseButton
	switch xbutton {
	case C.XBUTTON1:
		button = inputevent.MouseButtonMouse4
	case C.XBUTTON2:
		button = inputevent.MouseButtonMouse5
	}
	return button
}

// keyCodeToVirtualKey converts Windows virtual key codes as defined in https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes to [inputevent.KeyCode].
func keyCodeToVirtualKey(virtualKey C.DWORD) inputevent.KeyCode {

	// todo(kfj): codegen?

	switch virtualKey {
	case C.VK_ESCAPE:
		return inputevent.Escape

	case C.VK_F1:
		return inputevent.F1
	case C.VK_F2:
		return inputevent.F2
	case C.VK_F3:
		return inputevent.F3
	case C.VK_F4:
		return inputevent.F4
	case C.VK_F5:
		return inputevent.F5
	case C.VK_F6:
		return inputevent.F6
	case C.VK_F7:
		return inputevent.F7
	case C.VK_F8:
		return inputevent.F8
	case C.VK_F9:
		return inputevent.F9
	case C.VK_F10:
		return inputevent.F10
	case C.VK_F11:
		return inputevent.F11
	case C.VK_F12:
		return inputevent.F12

	case C.VK_SNAPSHOT:
		return inputevent.PrintScreen
	case C.VK_SCROLL:
		return inputevent.ScrollLock
	case C.VK_PAUSE:
		return inputevent.PauseBreak

	case C.VK_OEM_3:
		return inputevent.Grave

	case 0x31:
		return inputevent.D1
	case 0x32:
		return inputevent.D2
	case 0x33:
		return inputevent.D3
	case 0x34:
		return inputevent.D4
	case 0x35:
		return inputevent.D5
	case 0x36:
		return inputevent.D6
	case 0x37:
		return inputevent.D7
	case 0x38:
		return inputevent.D8
	case 0x39:
		return inputevent.D9
	case 0x30:
		return inputevent.D0

	case C.VK_OEM_MINUS:
		return inputevent.Minus
	case C.VK_OEM_PLUS:
		return inputevent.Equal

	case 0x41:
		return inputevent.A
	case 0x42:
		return inputevent.B
	case 0x43:
		return inputevent.C
	case 0x44:
		return inputevent.D
	case 0x45:
		return inputevent.E
	case 0x46:
		return inputevent.F
	case 0x47:
		return inputevent.G
	case 0x48:
		return inputevent.H
	case 0x49:
		return inputevent.I
	case 0x4A:
		return inputevent.J
	case 0x4B:
		return inputevent.K
	case 0x4C:
		return inputevent.L
	case 0x4D:
		return inputevent.M
	case 0x4E:
		return inputevent.N
	case 0x4F:
		return inputevent.O
	case 0x50:
		return inputevent.P
	case 0x51:
		return inputevent.Q
	case 0x52:
		return inputevent.R
	case 0x53:
		return inputevent.S
	case 0x54:
		return inputevent.T
	case 0x55:
		return inputevent.U
	case 0x56:
		return inputevent.V
	case 0x57:
		return inputevent.W
	case 0x58:
		return inputevent.X
	case 0x59:
		return inputevent.Y
	case 0x5A:
		return inputevent.Z

	case C.VK_OEM_4:
		return inputevent.LeftBrace
	case C.VK_OEM_6:
		return inputevent.RightBrace

	case C.VK_OEM_1:
		return inputevent.SemiColon
	case C.VK_OEM_7:
		return inputevent.Apostrophe

	case C.VK_OEM_COMMA:
		return inputevent.Comma
	case C.VK_OEM_PERIOD:
		return inputevent.Dot
	case C.VK_OEM_2:
		return inputevent.Slash

	case C.VK_BACK:
		return inputevent.Backspace
	case C.VK_OEM_5:
		return inputevent.BackSlash
	case C.VK_RETURN:
		return inputevent.Enter

	case C.VK_SPACE:
		return inputevent.Space

	case C.VK_TAB:
		return inputevent.Tab
	case C.VK_CAPITAL:
		return inputevent.CapsLock

	case C.VK_LSHIFT:
		return inputevent.LeftShift
	case C.VK_RSHIFT:
		return inputevent.RightShift

	case C.VK_LCONTROL:
		return inputevent.LeftCtrl
	case C.VK_RCONTROL:
		return inputevent.RightCtrl

	case C.VK_LMENU:
		return inputevent.LeftAlt
	case C.VK_RMENU:
		return inputevent.RightAlt

	case C.VK_LWIN:
		return inputevent.LeftMeta
	case C.VK_RWIN:
		return inputevent.RightMeta

	case C.VK_INSERT:
		return inputevent.Insert
	case C.VK_DELETE:
		return inputevent.Delete

	case C.VK_HOME:
		return inputevent.Home
	case C.VK_END:
		return inputevent.End

	case C.VK_PRIOR:
		return inputevent.PageUp
	case C.VK_NEXT:
		return inputevent.PageDown

	case C.VK_UP:
		return inputevent.Up
	case C.VK_LEFT:
		return inputevent.Left
	case C.VK_DOWN:
		return inputevent.Down
	case C.VK_RIGHT:
		return inputevent.Right
	}

	return 0
}
