#include <windows.h>

#include "hook_windows_amd64.h"

_Thread_local hook_event_t hook_event;

_Thread_local BOOL eat_input;

_Thread_local LONGLONG mouse_hook_proc_worst;

_Thread_local LONGLONG keyboard_hook_proc_worst;

void set_eat_input(BOOL flag)
{
    eat_input = flag;
}

LONGLONG get_mouse_hook_proc_worst()
{
    return mouse_hook_proc_worst;
}

LONGLONG get_keyboard_hook_proc_worst()
{
    return keyboard_hook_proc_worst;
}

LRESULT mouse_hook_proc(int nCode, WPARAM wParam, LPARAM lParam)
{
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    LONGLONG t0 = t.QuadPart;

    MSLLHOOKSTRUCT *details = (MSLLHOOKSTRUCT *)lParam;

    hook_event.code = wParam;

    switch (hook_event.code)
    {
    case WM_MOUSEMOVE:
        hook_event.data.mouse_move.x = details->pt.x;
        hook_event.data.mouse_move.y = details->pt.y;
        break;

    case WM_XBUTTONDOWN:
    case WM_XBUTTONUP:
        hook_event.data.mouse_click.button = (WORD)(details->mouseData >> 16);
        break;

    case WM_MOUSEWHEEL:
        hook_event.data.mouse_scroll.distance = (SHORT)(details->mouseData >> 16);
        break;
    }

    PostMessageW(NULL, MESSAGE_CODE_HOOK_EVENT, WH_MOUSE_LL, (LPARAM)NULL);

    QueryPerformanceCounter(&t);
    LONGLONG d = t.QuadPart / 1000 - t0 / 1000;
    if (d > mouse_hook_proc_worst)
    {
        mouse_hook_proc_worst = d;
    }

    if (eat_input)
    {
        return 1;
    }
    return CallNextHookEx(NULL, nCode, wParam, lParam);
}

LRESULT keyboard_hook_proc(int nCode, WPARAM wParam, LPARAM lParam)
{
    LARGE_INTEGER t;
    QueryPerformanceCounter(&t);
    LONGLONG t0 = t.QuadPart;

    KBDLLHOOKSTRUCT *details = (KBDLLHOOKSTRUCT *)lParam;

    hook_event.code = wParam;

    switch (hook_event.code)
    {
    case WM_KEYDOWN:
    case WM_KEYUP:
    case WM_SYSKEYDOWN:
    case WM_SYSKEYUP:
        hook_event.data.key_press.virtual_key = details->vkCode;
        break;
    }

    PostMessageW(NULL, MESSAGE_CODE_HOOK_EVENT, WH_KEYBOARD_LL, (LPARAM)NULL);

    QueryPerformanceCounter(&t);
    LONGLONG d = t.QuadPart / 1000 - t0 / 1000;
    if (d > keyboard_hook_proc_worst)
    {
        keyboard_hook_proc_worst = d;
    }

    if (eat_input)
    {
        return 1;
    }
    return CallNextHookEx(NULL, nCode, wParam, lParam);
}

hook_event_t *get_hook_event()
{
    return &hook_event;
}

BOOL get_message(LPMSG lpMsg)
{
    return GetMessageW(lpMsg, (HWND)-1, 0, 0);
}
