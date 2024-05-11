#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
#include <windows.h>
#include <winuser.h>

#include "inputhook_windows_amd64.h"

_Thread_local BOOL should_consume;

void reset_thread_local()
{
    should_consume = FALSE;
}

void set_should_consume(BOOL flag)
{
    should_consume = flag;
}

LRESULT mouse_hook_proc(int nCode, WPARAM wParam, LPARAM lParam)
{
    MSLLHOOKSTRUCT *details = (MSLLHOOKSTRUCT *)lParam;

    input_event_t *input_event = calloc(1, sizeof(input_event_t));
    input_event->code = wParam;

    switch (input_event->code)
    {
    case WM_MOUSEMOVE:
        input_event->data.mouse_move.x = details->pt.x;
        input_event->data.mouse_move.y = details->pt.y;
        break;

    case WM_MOUSEWHEEL:
        input_event->data.mouse_scroll.distance = (int16_t)(details->mouseData >> 16);
        break;
    }

    if (!PostMessageW(NULL, MESSAGE_CODE_INPUT_EVENT, WH_MOUSE_LL, (LPARAM)input_event))
    {
        free(input_event);
    }

    if (!should_consume)
    {
        return CallNextHookEx(NULL, nCode, wParam, lParam);
    }
    return 1;
}

LRESULT keyboard_hook_proc(int nCode, WPARAM wParam, LPARAM lParam)
{
    KBDLLHOOKSTRUCT *details = (KBDLLHOOKSTRUCT *)lParam;

    input_event_t *input_event = calloc(1, sizeof(input_event_t));
    input_event->code = wParam;

    switch (input_event->code)
    {
    }

    if (!PostMessageW(NULL, MESSAGE_CODE_INPUT_EVENT, WH_KEYBOARD_LL, (LPARAM)input_event))
    {
        free(input_event);
    }

    if (!should_consume)
    {
        return CallNextHookEx(NULL, nCode, wParam, lParam);
    }
    return 1;
}

input_event_t *get_input_event(LONG_PTR ptr)
{
    input_event_t *event = (input_event_t *)ptr;
    return event;
}

void free_input_event(LONG_PTR ptr)
{
    input_event_t *event = (input_event_t *)ptr;
    free(event);
}
