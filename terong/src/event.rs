use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum InputEvent {
    MousePosition(MousePosition),
    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },
    MouseScroll {},

    KeyDown { key: KeyCode },
    KeyUp { key: KeyCode },
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum MouseButton {
    Left = 0,
    Right,
    Middle,
    Mouse4,
    Mouse5,
}

/// Keyboard key.
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
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

    Tilde,

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
    Plus,

    Backspace,

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

    Space,

    Enter,

    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,

    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

impl KeyCode {
    pub unsafe fn from_u16(n: u16) -> Self {
        std::mem::transmute(n)
    }
}
