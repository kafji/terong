use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum InputEvent {
    MouseMove { dx: i32, dy: i32 },
    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },
    MouseScroll { direction: ScrollDirection },

    KeyDown { key: KeyboardKey },
    KeyUp { key: KeyboardKey },
}

#[repr(u8)]
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum MouseButton {
    Left = 0,
    Right,
    Middle,
    Mouse4,
    Mouse5,
}

#[repr(u8)]
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum ScrollDirection {
    Up = 0,
    Down,
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum KeyboardKey {
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
    LeftCtrl,
    LeftAlt,
    LeftSys,
    Space,
    RightSys,
    RightAlt,
    RightCtrl,
    RightShift,
    Enter,
}

impl KeyboardKey {
    pub unsafe fn from_u32(n: u32) -> Self {
        assert!(n < Self::Enter as u32 + 1);
        std::mem::transmute(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        unsafe {
            let k = KeyboardKey::from_u32(81);
            assert_eq!(k, KeyboardKey::B);
        }
    }
}
