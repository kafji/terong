#ifndef HOOK
#define HOOK

#define MESSAGE_CODE_HOOK_EVENT WM_APP
#define MESSAGE_CODE_CONTROL_COMMAND WM_APP + 1
#define MESSAGE_CODE_SET_CAPTURE_INPUTS WM_APP + 2

#define CONTROL_COMMAND_STOP 1

typedef struct
{
    LONG x;
    LONG y;
} mouse_move_t;

typedef struct
{
    WORD button;
} mouse_click_t;

typedef struct
{
    SHORT distance;
} mouse_scroll_t;

typedef struct
{
    DWORD virtual_key;
} key_press_t;

typedef union
{
    mouse_move_t mouse_move;
    mouse_click_t mouse_click;
    mouse_scroll_t mouse_scroll;
    key_press_t key_press;
} hook_event_data_t;

typedef struct
{
    WPARAM code;
    hook_event_data_t data;
} hook_event_t;

hook_event_t *get_hook_event();

LRESULT mouse_hook_proc(int nCode, WPARAM wParam, LPARAM lParam);

LRESULT keyboard_hook_proc(int nCode, WPARAM wParam, LPARAM lParam);

void set_eat_input(BOOL flag);

LONGLONG get_mouse_hook_proc_worst();

LONGLONG get_keyboard_hook_proc_worst();

BOOL get_message(LPMSG lpMsg);

#endif
