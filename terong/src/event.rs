use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum InputEvent {
    MouseMove { dx: i32, dy: i32 },
    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },
    MouseScroll {},

    KeyDown { key: Key },
    KeyUp { key: Key },
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
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum Key {
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

    LeftSys,
    RightSys,

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

impl Key {
    pub unsafe fn from_u32(n: u32) -> Self {
        assert!(n < Self::ArrowRight as u32 + 1);
        std::mem::transmute(n)
    }
}
