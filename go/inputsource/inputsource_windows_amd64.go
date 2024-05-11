package inputsource

/*
#include <windows.h>
#include <winuser.h>
#include <processthreadsapi.h>

#include "inputhook_windows_amd64.h"
*/
import "C"

import (
	"runtime"
	"sync"
	"unsafe"

	"kafji.net/terong/inputevent"
)

type Handle struct {
	mu           sync.Mutex
	threadID     C.DWORD
	moduleHandle C.HMODULE
}

func Start(sink chan<- inputevent.InputEvent) *Handle {
	h := &Handle{}
	go run(h, sink)
	return h
}

func (h *Handle) Stop() {
	h.mu.Lock()
	defer h.mu.Unlock()

	C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_CONTROL_COMMAND, C.CONTROL_COMMAND_STOP, 0)
}

func (h *Handle) SetShouldConsume(flag bool) {
	h.mu.Lock()
	defer h.mu.Unlock()

	if flag {
		C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_SHOULD_CONSUME, C.TRUE, 0)
	} else {
		C.PostThreadMessageW(h.threadID, C.MESSAGE_CODE_SET_SHOULD_CONSUME, C.FALSE, 0)
	}
}

func run(h *Handle, sink chan<- inputevent.InputEvent) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()

	C.reset_thread_local()

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

	// https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues
loop:
	for {
		var msg C.MSG
		ret := C.GetMessageW(&msg, nil, 0, 0)
		if ret == -1 {
			break
		}

		switch msg.message {
		case C.MESSAGE_CODE_INPUT_EVENT:
			var event inputevent.InputEvent

			switch msg.wParam {
			case C.WH_MOUSE_LL:
				func() {
					defer C.free_input_event(msg.lParam)

					internal := C.get_input_event(msg.lParam)
					switch internal.code {
					case C.WM_MOUSEMOVE:
						data := (*C.mouse_move_t)(unsafe.Pointer(&internal.data))
						_ = data
						event.Data = inputevent.MouseMove{}
					case C.WM_LBUTTONDOWN:
						event.Data = inputevent.MouseClick{Button: inputevent.LEFT, Action: inputevent.ACTION_DOWN}
					case C.WM_LBUTTONUP:
						event.Data = inputevent.MouseClick{Button: inputevent.LEFT, Action: inputevent.ACTION_UP}
					case C.WM_RBUTTONDOWN:
						event.Data = inputevent.MouseClick{Button: inputevent.RIGHT, Action: inputevent.ACTION_DOWN}
					case C.WM_RBUTTONUP:
						event.Data = inputevent.MouseClick{Button: inputevent.RIGHT, Action: inputevent.ACTION_UP}
					case C.WM_MBUTTONDOWN:
						event.Data = inputevent.MouseClick{Button: inputevent.MIDDLE, Action: inputevent.ACTION_DOWN}
					case C.WM_MBUTTONUP:
						event.Data = inputevent.MouseClick{Button: inputevent.MIDDLE, Action: inputevent.ACTION_UP}
					case C.WM_MOUSEWHEEL:
						data := (*C.mouse_scroll_t)(unsafe.Pointer(&internal.data))
						count := int(data.distance) / int(C.WHEEL_DELTA)
						switch {
						case count > 0:
							event.Data = inputevent.MouseScroll{Count: uint8(count), Direction: inputevent.SCROLL_UP}
						case count < 0:
							event.Data = inputevent.MouseScroll{Count: uint8(count * -1), Direction: inputevent.SCROLL_DOWN}
						case count == 0:
						}
					}
				}()

			case C.WH_KEYBOARD_LL:
				func() {
					defer C.free_input_event(msg.lParam)

				}()
			}

			if event != (inputevent.InputEvent{}) {
				sink <- event
			}

		case C.MESSAGE_CODE_CONTROL_COMMAND:
			switch msg.wParam {
			case C.CONTROL_COMMAND_STOP:
				break loop
			}

		case C.MESSAGE_CODE_SET_SHOULD_CONSUME:
			C.set_should_consume(C.BOOL(msg.wParam))
		}

		C.DispatchMessageW(&msg)
	}
}
