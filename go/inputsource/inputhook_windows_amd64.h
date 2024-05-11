#ifndef INPUT_HOOK
#define INPUT_HOOK

typedef struct
{
    int x;
    int y;
} mouse_move_t;

typedef struct
{
    int distance;
} mouse_scroll_t;

typedef union
{
    mouse_move_t mouse_move;
    mouse_scroll_t mouse_scroll;
} input_event_data_t;

typedef struct
{
    int code;
    input_event_data_t data;
} input_event_t;

input_event_t *get_input_event(LONG_PTR ptr);

void free_input_event(LONG_PTR ptr);

LRESULT mouse_hook_proc(int nCode, WPARAM wParam, LPARAM lParam);

LRESULT keyboard_hook_proc(int nCode, WPARAM wParam, LPARAM lParam);

void reset_thread_local();

void set_should_consume(BOOL flag);

#define MESSAGE_CODE_INPUT_EVENT WM_APP

#define MESSAGE_CODE_CONTROL_COMMAND WM_APP + 1

#define MESSAGE_CODE_SET_SHOULD_CONSUME WM_APP + 2

#define CONTROL_COMMAND_STOP 0

#endif
