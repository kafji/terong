#include <windows.h>
#include <winuser.h>

#include "hook_windows_amd64.h"

_Thread_local input_event_t input_event;

_Thread_local BOOL should_eat_input;

void set_should_eat_input(BOOL flag)
{
    should_eat_input = flag;
}

BOOL get_should_eat_input()
{
    return should_eat_input;
}

LRESULT mouse_hook_proc(int nCode, WPARAM wParam, LPARAM lParam)
{
    MSLLHOOKSTRUCT *details = (MSLLHOOKSTRUCT *)lParam;

    input_event.code = wParam;

    switch (input_event.code)
    {
    case WM_MOUSEMOVE:
        input_event.data.mouse_move.x = details->pt.x;
        input_event.data.mouse_move.y = details->pt.y;
        break;

    case WM_XBUTTONDOWN:
    case WM_XBUTTONUP:
        input_event.data.mouse_click.button = (WORD)(details->mouseData >> 16);
        break;

    case WM_MOUSEWHEEL:
        input_event.data.mouse_scroll.distance = (SHORT)(details->mouseData >> 16);
        break;
    }

    PostMessageW(NULL, MESSAGE_CODE_INPUT_EVENT, WH_MOUSE_LL, (LPARAM)NULL);

    if (should_eat_input)
    {
        return 1;
    }
    return CallNextHookEx(NULL, nCode, wParam, lParam);
}

LRESULT keyboard_hook_proc(int nCode, WPARAM wParam, LPARAM lParam)
{
    KBDLLHOOKSTRUCT *details = (KBDLLHOOKSTRUCT *)lParam;

    input_event.code = wParam;

    switch (input_event.code)
    {
    case WM_KEYDOWN:
    case WM_KEYUP:
    case WM_SYSKEYDOWN:
    case WM_SYSKEYUP:
        input_event.data.key_press.virtual_key = details->vkCode;
        break;
    }

    PostMessageW(NULL, MESSAGE_CODE_INPUT_EVENT, WH_KEYBOARD_LL, (LPARAM)NULL);

    if (should_eat_input)
    {
        return 1;
    }
    return CallNextHookEx(NULL, nCode, wParam, lParam);
}

input_event_t *get_input_event()
{
    return &input_event;
}
