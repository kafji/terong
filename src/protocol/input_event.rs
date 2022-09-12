use serde::{Deserialize, Serialize};
use strum::{EnumIter, FromRepr};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum InputEvent {
    MouseMove { dx: i32, dy: i32 },

    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },

    MouseScroll {},

    KeyDown { key: KeyCode },
    KeyUp { key: KeyCode },
}

#[repr(u8)]
#[derive(FromRepr, EnumIter, Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum MouseButton {
    Left = 0,
    Right,
    Middle,
    Mouse4,
    Mouse5,
}

/// Keyboard key.
#[repr(u16)]
#[derive(FromRepr, EnumIter, Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum KeyCode {
    Escape = 0,

    // function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    PrintScreen,
    ScrollLock,
    PauseBreak,

    /// The tilde key.
    Grave,

    // digits
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    D0,

    Minus,
    Equal,

    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    LeftBrace,
    RightBrace,

    SemiColon,
    Apostrophe,

    Comma,
    Dot,
    Slash,

    Backspace,
    BackSlash,
    Enter,

    Space,

    Tab,
    CapsLock,

    LeftShift,
    RightShift,

    LeftCtrl,
    RightCtrl,

    LeftAlt,
    RightAlt,

    LeftMeta,
    RightMeta,

    Insert,
    Delete,

    Home,
    End,

    PageUp,
    PageDown,

    Up,
    Left,
    Down,
    Right,
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use evdev_rs::enums::EV_KEY;

    impl Into<EV_KEY> for KeyCode {
        fn into(self) -> EV_KEY {
            use KeyCode::*;
            use EV_KEY::*;
            match self {
                Escape => KEY_ESC,

                F1 => KEY_F1,
                F2 => KEY_F2,
                F3 => KEY_F3,
                F4 => KEY_F4,
                F5 => KEY_F5,
                F6 => KEY_F6,
                F7 => KEY_F7,
                F8 => KEY_F8,
                F9 => KEY_F9,
                F10 => KEY_F10,
                F11 => KEY_F11,
                F12 => KEY_F12,

                PrintScreen => KEY_PRINT,
                ScrollLock => KEY_SCROLLLOCK,
                PauseBreak => KEY_PAUSE,

                Grave => KEY_GRAVE,

                D1 => KEY_1,
                D2 => KEY_2,
                D3 => KEY_3,
                D4 => KEY_4,
                D5 => KEY_5,
                D6 => KEY_6,
                D7 => KEY_7,
                D8 => KEY_8,
                D9 => KEY_9,
                D0 => KEY_0,

                Minus => KEY_MINUS,
                Equal => KEY_EQUAL,

                A => KEY_A,
                B => KEY_B,
                C => KEY_C,
                D => KEY_D,
                E => KEY_E,
                F => KEY_F,
                G => KEY_G,
                H => KEY_H,
                I => KEY_I,
                J => KEY_J,
                K => KEY_K,
                L => KEY_L,
                M => KEY_M,
                N => KEY_N,
                O => KEY_O,
                P => KEY_P,
                Q => KEY_Q,
                R => KEY_R,
                S => KEY_S,
                T => KEY_T,
                U => KEY_U,
                V => KEY_V,
                W => KEY_W,
                X => KEY_X,
                Y => KEY_Y,
                Z => KEY_Z,

                LeftBrace => KEY_LEFTBRACE,
                RightBrace => KEY_RIGHTBRACE,

                SemiColon => KEY_SEMICOLON,
                Apostrophe => KEY_APOSTROPHE,

                Comma => KEY_COMMA,
                Dot => KEY_DOT,
                Slash => KEY_SLASH,

                Backspace => KEY_BACKSPACE,
                BackSlash => KEY_BACKSLASH,
                Enter => KEY_ENTER,

                Space => KEY_SPACE,

                Tab => EV_KEY::KEY_TAB,
                CapsLock => KEY_CAPSLOCK,

                LeftShift => KEY_LEFTSHIFT,
                RightShift => KEY_RIGHTSHIFT,

                LeftCtrl => KEY_LEFTALT,
                RightCtrl => KEY_RIGHTCTRL,

                LeftAlt => KEY_LEFTALT,
                RightAlt => KEY_RIGHTALT,

                LeftMeta => KEY_LEFTMETA,
                RightMeta => KEY_RIGHTMETA,

                Insert => KEY_INSERT,
                Delete => KEY_DELETE,

                Home => KEY_HOME,
                End => KEY_END,

                PageUp => KEY_PAGEUP,
                PageDown => KEY_PAGEDOWN,

                Up => KEY_UP,
                Left => KEY_LEFT,
                Down => KEY_DOWN,
                Right => KEY_RIGHT,
            }
        }
    }

    impl Into<EV_KEY> for MouseButton {
        fn into(self) -> EV_KEY {
            match self {
                MouseButton::Left => EV_KEY::BTN_LEFT,
                MouseButton::Right => EV_KEY::BTN_RIGHT,
                MouseButton::Middle => EV_KEY::BTN_MIDDLE,
                MouseButton::Mouse4 => EV_KEY::BTN_4,
                MouseButton::Mouse5 => EV_KEY::BTN_5,
            }
        }
    }
}
