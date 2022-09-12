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
