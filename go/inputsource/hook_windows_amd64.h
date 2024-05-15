#ifndef HOOK
#define HOOK

#define MESSAGE_CODE_INPUT_EVENT WM_APP
#define MESSAGE_CODE_CONTROL_COMMAND WM_APP + 1
#define MESSAGE_CODE_SET_SHOULD_EAT_INPUT WM_APP + 2
#define MESSAGE_CODE_SET_CAPTURE_MOUSE_MOVE WM_APP + 3

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
} input_event_data_t;

typedef struct
{
    WPARAM code;
    input_event_data_t data;
} input_event_t;

input_event_t *get_input_event();

LRESULT mouse_hook_proc(int nCode, WPARAM wParam, LPARAM lParam);

LRESULT keyboard_hook_proc(int nCode, WPARAM wParam, LPARAM lParam);

void set_should_eat_input(BOOL flag);

BOOL get_should_eat_input();

#endif
