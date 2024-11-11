package inputsource

/*
#cgo CFLAGS: -Wall -g -O2
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
}

func Start() *Handle {
	h := &Handle{inputs: make(chan inputevent.InputEvent, 10_000)}
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

	normalizer := inputevent.Normalizer{}

	screenCenter, err := screenCenter()
	if err != nil {
		return err
	}

	var oldCursorPos *C.POINT

	var oldMouseHookProcWorst uint64
	var oldKeyboardHookProcWorst uint64

	// https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues
	for count := uint(1); ; count++ {
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

		// in case previous loop produce error
		if err := windows.GetLastError(); err != nil {
			return err
		}

		// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessagew
		var msg C.MSG
		ret := C.get_message(&msg)
		if ret < 0 {
			return windows.GetLastError()
		}
		if ret == 0 {
			return nil
		}

		// sample every hundred or so messages
		if count%128 == 0 {
			mouseWorst := uint64(C.get_mouse_hook_proc_worst())
			if mouseWorst > 50 && mouseWorst > oldMouseHookProcWorst {
				slog.Warn("mouse hook proc worst latency increased", "latency_ms", mouseWorst)
				oldMouseHookProcWorst = mouseWorst
			}

			keyboardWorst := uint64(C.get_keyboard_hook_proc_worst())
			if keyboardWorst > 50 && keyboardWorst > oldKeyboardHookProcWorst {
				slog.Warn("keyboard hook proc worst latency increased", "latency_ms", keyboardWorst)
				oldKeyboardHookProcWorst = keyboardWorst
			}
		}

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
					dx := data.x - C.LONG(screenCenter.x)
					dy := -(data.y - C.LONG(screenCenter.y))
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
					key := keyCodeToVirtualKey[data.virtual_key]
					input = inputevent.KeyPress{Key: key, Action: inputevent.KeyActionDown}

				case C.WM_KEYUP:
					fallthrough
				case C.WM_SYSKEYUP:
					data := (*C.key_press_t)(unsafe.Pointer(&hookEvent.data))
					key := keyCodeToVirtualKey[data.virtual_key]
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
				// capture current mouse position
				oldCursorPos = &C.POINT{}
				ret := C.GetCursorPos(oldCursorPos)
				if ret == 0 {
					return windows.GetLastError()
				}
				// set mouse position to center of screen
				ret = C.SetCursorPos(C.int(screenCenter.x), C.int(screenCenter.y))
				if ret == 0 {
					return windows.GetLastError()
				}
			} else if oldCursorPos != nil {
				// restore previous mouse position
				ret := C.SetCursorPos(C.int(oldCursorPos.x), C.int(oldCursorPos.y))
				if ret == 0 {
					return windows.GetLastError()
				}
				oldCursorPos = nil
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

// Table to convert Windows virtual key codes as defined in https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes to [inputevent.KeyCode].
var keyCodeToVirtualKey = make([]inputevent.KeyCode, 0xFF)

func init() {
	keyCodeToVirtualKey[C.VK_ESCAPE] = inputevent.Escape

	keyCodeToVirtualKey[C.VK_F1] = inputevent.F1
	keyCodeToVirtualKey[C.VK_F2] = inputevent.F2
	keyCodeToVirtualKey[C.VK_F3] = inputevent.F3
	keyCodeToVirtualKey[C.VK_F4] = inputevent.F4
	keyCodeToVirtualKey[C.VK_F5] = inputevent.F5
	keyCodeToVirtualKey[C.VK_F6] = inputevent.F6
	keyCodeToVirtualKey[C.VK_F7] = inputevent.F7
	keyCodeToVirtualKey[C.VK_F8] = inputevent.F8
	keyCodeToVirtualKey[C.VK_F9] = inputevent.F9
	keyCodeToVirtualKey[C.VK_F10] = inputevent.F10
	keyCodeToVirtualKey[C.VK_F11] = inputevent.F11
	keyCodeToVirtualKey[C.VK_F12] = inputevent.F12

	keyCodeToVirtualKey[C.VK_SNAPSHOT] = inputevent.PrintScreen
	keyCodeToVirtualKey[C.VK_SCROLL] = inputevent.ScrollLock
	keyCodeToVirtualKey[C.VK_PAUSE] = inputevent.PauseBreak

	keyCodeToVirtualKey[C.VK_OEM_3] = inputevent.Grave

	keyCodeToVirtualKey[0x31] = inputevent.D1
	keyCodeToVirtualKey[0x32] = inputevent.D2
	keyCodeToVirtualKey[0x33] = inputevent.D3
	keyCodeToVirtualKey[0x34] = inputevent.D4
	keyCodeToVirtualKey[0x35] = inputevent.D5
	keyCodeToVirtualKey[0x36] = inputevent.D6
	keyCodeToVirtualKey[0x37] = inputevent.D7
	keyCodeToVirtualKey[0x38] = inputevent.D8
	keyCodeToVirtualKey[0x39] = inputevent.D9
	keyCodeToVirtualKey[0x30] = inputevent.D0

	keyCodeToVirtualKey[C.VK_OEM_MINUS] = inputevent.Minus
	keyCodeToVirtualKey[C.VK_OEM_PLUS] = inputevent.Equal

	keyCodeToVirtualKey[0x41] = inputevent.A
	keyCodeToVirtualKey[0x42] = inputevent.B
	keyCodeToVirtualKey[0x43] = inputevent.C
	keyCodeToVirtualKey[0x44] = inputevent.D
	keyCodeToVirtualKey[0x45] = inputevent.E
	keyCodeToVirtualKey[0x46] = inputevent.F
	keyCodeToVirtualKey[0x47] = inputevent.G
	keyCodeToVirtualKey[0x48] = inputevent.H
	keyCodeToVirtualKey[0x49] = inputevent.I
	keyCodeToVirtualKey[0x4A] = inputevent.J
	keyCodeToVirtualKey[0x4B] = inputevent.K
	keyCodeToVirtualKey[0x4C] = inputevent.L
	keyCodeToVirtualKey[0x4D] = inputevent.M
	keyCodeToVirtualKey[0x4E] = inputevent.N
	keyCodeToVirtualKey[0x4F] = inputevent.O
	keyCodeToVirtualKey[0x50] = inputevent.P
	keyCodeToVirtualKey[0x51] = inputevent.Q
	keyCodeToVirtualKey[0x52] = inputevent.R
	keyCodeToVirtualKey[0x53] = inputevent.S
	keyCodeToVirtualKey[0x54] = inputevent.T
	keyCodeToVirtualKey[0x55] = inputevent.U
	keyCodeToVirtualKey[0x56] = inputevent.V
	keyCodeToVirtualKey[0x57] = inputevent.W
	keyCodeToVirtualKey[0x58] = inputevent.X
	keyCodeToVirtualKey[0x59] = inputevent.Y
	keyCodeToVirtualKey[0x5A] = inputevent.Z

	keyCodeToVirtualKey[C.VK_OEM_4] = inputevent.LeftBrace
	keyCodeToVirtualKey[C.VK_OEM_6] = inputevent.RightBrace

	keyCodeToVirtualKey[C.VK_OEM_1] = inputevent.SemiColon
	keyCodeToVirtualKey[C.VK_OEM_7] = inputevent.Apostrophe

	keyCodeToVirtualKey[C.VK_OEM_COMMA] = inputevent.Comma
	keyCodeToVirtualKey[C.VK_OEM_PERIOD] = inputevent.Dot
	keyCodeToVirtualKey[C.VK_OEM_2] = inputevent.Slash

	keyCodeToVirtualKey[C.VK_BACK] = inputevent.Backspace
	keyCodeToVirtualKey[C.VK_OEM_5] = inputevent.BackSlash
	keyCodeToVirtualKey[C.VK_RETURN] = inputevent.Enter

	keyCodeToVirtualKey[C.VK_SPACE] = inputevent.Space

	keyCodeToVirtualKey[C.VK_TAB] = inputevent.Tab
	keyCodeToVirtualKey[C.VK_CAPITAL] = inputevent.CapsLock

	keyCodeToVirtualKey[C.VK_LSHIFT] = inputevent.LeftShift
	keyCodeToVirtualKey[C.VK_RSHIFT] = inputevent.RightShift

	keyCodeToVirtualKey[C.VK_LCONTROL] = inputevent.LeftCtrl
	keyCodeToVirtualKey[C.VK_RCONTROL] = inputevent.RightCtrl

	keyCodeToVirtualKey[C.VK_LMENU] = inputevent.LeftAlt
	keyCodeToVirtualKey[C.VK_RMENU] = inputevent.RightAlt

	keyCodeToVirtualKey[C.VK_LWIN] = inputevent.LeftMeta
	keyCodeToVirtualKey[C.VK_RWIN] = inputevent.RightMeta

	keyCodeToVirtualKey[C.VK_INSERT] = inputevent.Insert
	keyCodeToVirtualKey[C.VK_DELETE] = inputevent.Delete

	keyCodeToVirtualKey[C.VK_HOME] = inputevent.Home
	keyCodeToVirtualKey[C.VK_END] = inputevent.End

	keyCodeToVirtualKey[C.VK_PRIOR] = inputevent.PageUp
	keyCodeToVirtualKey[C.VK_NEXT] = inputevent.PageDown

	keyCodeToVirtualKey[C.VK_UP] = inputevent.Up
	keyCodeToVirtualKey[C.VK_LEFT] = inputevent.Left
	keyCodeToVirtualKey[C.VK_DOWN] = inputevent.Down
	keyCodeToVirtualKey[C.VK_RIGHT] = inputevent.Right
}
