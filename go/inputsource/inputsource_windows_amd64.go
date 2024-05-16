package inputsource

/*
#include <errhandlingapi.h>
#include <processthreadsapi.h>
#include <windows.h>
#include <winuser.h>

#include "hook_windows_amd64.h"
*/
import "C"

import (
	"log/slog"
	"runtime"
	"sync"
	"unsafe"

	"kafji.net/terong/inputevent"
)

type Handle struct {
	mu           sync.Mutex
	threadID     C.DWORD
	moduleHandle C.HMODULE
	stopped      bool

	captureMouseMove bool
}

func Start(sink chan<- any) *Handle {
	h := &Handle{}
	go run(h, sink)
	return h
}

func (h *Handle) Stop() {
	for {
		h.mu.Lock()
		done := false
		switch {
		case h.stopped:
			done = true
		case h.threadID == 0:
			slog.Debug("thread id is unset")
		default:
			C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_CONTROL_COMMAND, C.CONTROL_COMMAND_STOP, 0)
			done = true
		}
		h.mu.Unlock()
		if done {
			break
		}
	}
}

func (h *Handle) SetShouldEatInput(flag bool) {
	for {
		h.mu.Lock()
		done := false
		switch {
		case h.threadID == 0:
			slog.Debug("thread id is unset")
		default:
			if flag {
				C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_SHOULD_EAT_INPUT, C.TRUE, 0)
			} else {
				C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_SHOULD_EAT_INPUT, C.FALSE, 0)
			}
			done = true
		}
		h.mu.Unlock()
		if done {
			break
		}
	}
}

func (h *Handle) SetCaptureMouseMove(flag bool) {
	for {
		h.mu.Lock()
		done := false
		switch {
		case h.threadID == 0:
			slog.Debug("thread id is unset")
		default:
			if flag {
				C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_CAPTURE_MOUSE_MOVE, C.TRUE, 0)
			} else {
				C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_CAPTURE_MOUSE_MOVE, C.FALSE, 0)
			}
			done = true
		}
		h.mu.Unlock()
		if done {
			break
		}
	}
}

func run(h *Handle, sink chan<- any) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	// todo(kfj): handle errors from win32 calls
	// https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-getlasterror

	var mouseHook C.HHOOK
	var keyboardHook C.HHOOK

	defer func() {
		if mouseHook != nil {
			C.UnhookWindowsHookEx(mouseHook)
		}
		if keyboardHook != nil {
			C.UnhookWindowsHookEx(keyboardHook)
		}
	}()

	func() {
		h.mu.Lock()
		defer h.mu.Unlock()

		h.threadID = C.GetCurrentThreadId()

		C.GetModuleHandleEx(0, nil, &h.moduleHandle)

		// https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc
		mouseHook = C.SetWindowsHookExW(C.WH_MOUSE_LL, (*[0]byte)(C.mouse_hook_proc), h.moduleHandle, 0)

		// https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc
		keyboardHook = C.SetWindowsHookExW(C.WH_KEYBOARD_LL, (*[0]byte)(C.keyboard_hook_proc), h.moduleHandle, 0)
	}()

	screen := screenSize()
	center := point{x: screen.x / 2, y: screen.y / 2}

	normalizer := inputevent.Normalizer{}

	// https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues
loop:
	for {
		if h.captureMouseMove {
			// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setcursorpos
			C.SetCursorPos(C.int(center.x), C.int(center.y))
		}

		var msg C.MSG
		ret := C.GetMessageW(&msg, nil, 0, 0)
		if ret == -1 {
			break
		}

		switch msg.message {

		case C.MESSAGE_CODE_INPUT_EVENT:
			input := C.get_input_event()

			var event any

			switch msg.wParam {

			case C.WH_MOUSE_LL:
				switch input.code {

				case C.WM_MOUSEMOVE:
					if !h.captureMouseMove {
						continue
					}
					data := (*C.mouse_move_t)(unsafe.Pointer(&input.data))
					dx := data.x - C.LONG(center.x)
					dy := (data.y - C.LONG(center.y)) * -1
					event = inputevent.MouseMove{DX: int16(dx), DY: int16(dy)}

				case C.WM_LBUTTONDOWN:
					event = inputevent.MouseClick{Button: inputevent.MouseButtonLeft, Action: inputevent.MouseButtonActionDown}

				case C.WM_LBUTTONUP:
					event = inputevent.MouseClick{Button: inputevent.MouseButtonLeft, Action: inputevent.MouseButtonActionUp}

				case C.WM_RBUTTONDOWN:
					event = inputevent.MouseClick{Button: inputevent.MouseButtonRight, Action: inputevent.MouseButtonActionDown}

				case C.WM_RBUTTONUP:
					event = inputevent.MouseClick{Button: inputevent.MouseButtonRight, Action: inputevent.MouseButtonActionUp}

				case C.WM_MBUTTONDOWN:
					event = inputevent.MouseClick{Button: inputevent.MouseButtonMiddle, Action: inputevent.MouseButtonActionDown}

				case C.WM_MBUTTONUP:
					event = inputevent.MouseClick{Button: inputevent.MouseButtonMiddle, Action: inputevent.MouseButtonActionUp}

				case C.WM_XBUTTONDOWN:
					data := (*C.mouse_click_t)(unsafe.Pointer(&input.data))
					button := xbuttonToMouseButton(data.button)
					if button != 0 {
						event = inputevent.MouseClick{Button: button, Action: inputevent.MouseButtonActionDown}
					}

				case C.WM_XBUTTONUP:
					data := (*C.mouse_click_t)(unsafe.Pointer(&input.data))
					button := xbuttonToMouseButton(data.button)
					if button != 0 {
						event = inputevent.MouseClick{Button: button, Action: inputevent.MouseButtonActionUp}
					}

				case C.WM_MOUSEWHEEL:
					data := (*C.mouse_scroll_t)(unsafe.Pointer(&input.data))
					count := int(data.distance) / int(C.WHEEL_DELTA)
					switch {
					case count > 0:
						event = inputevent.MouseScroll{Count: uint8(count), Direction: inputevent.MOUSE_SCROLL_UP}
					case count < 0:
						event = inputevent.MouseScroll{Count: uint8(count * -1), Direction: inputevent.MOUSE_SCROLL_DOWN}
					case count == 0:
					}
				}

			case C.WH_KEYBOARD_LL:
				switch input.code {

				case C.WM_KEYDOWN:
					fallthrough
				case C.WM_SYSKEYDOWN:
					data := (*C.key_press_t)(unsafe.Pointer(&input.data))
					key := keyCodeToVirtualKey(data.virtual_key)
					event = inputevent.KeyPress{Key: key, Action: inputevent.KeyActionDown}

				case C.WM_KEYUP:
					fallthrough
				case C.WM_SYSKEYUP:
					data := (*C.key_press_t)(unsafe.Pointer(&input.data))
					key := keyCodeToVirtualKey(data.virtual_key)
					event = inputevent.KeyPress{Key: key, Action: inputevent.KeyActionUp}
				}
			}

			if event != nil {
				event = normalizer.Normalize(event)
				// if the message pump blocked, user's input e.g. their mouse movements and key strokes, will get choppy
				select {
				case sink <- event:
				default:
					slog.Warn("dropping event, channel was blocked", "event", event)
				}
			}

		case C.MESSAGE_CODE_CONTROL_COMMAND:
			switch msg.wParam {
			case C.CONTROL_COMMAND_STOP:
				h.mu.Lock()
				h.stopped = true
				h.mu.Unlock()
				break loop
			}

		case C.MESSAGE_CODE_SET_SHOULD_EAT_INPUT:
			var flag bool
			switch C.BOOL(msg.wParam) {
			case C.TRUE:
				flag = true
			case C.FALSE:
				flag = false
			}
			slog.Info("setting should eat input", "value", flag)
			C.set_should_eat_input(C.BOOL(msg.wParam))

		case C.MESSAGE_CODE_SET_CAPTURE_MOUSE_MOVE:
			var flag bool
			switch C.BOOL(msg.wParam) {
			case C.TRUE:
				flag = true
			case C.FALSE:
				flag = false
			}
			slog.Info("setting capture mouse move", "value", flag)
			h.captureMouseMove = flag

		default:
			// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-dispatchmessagew
			C.DispatchMessageW(&msg)
		}
	}
}

type point struct {
	x uint16
	y uint16
}

func screenSize() point {
	rect := C.RECT{}
	// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow
	C.SystemParametersInfoW(C.SPI_GETWORKAREA, 0, C.PVOID(&rect), 0)
	return point{x: uint16(rect.right - rect.left), y: uint16(rect.bottom - rect.top)}
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
