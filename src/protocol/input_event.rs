use serde::{Deserialize, Serialize};
use strum::{EnumIter, FromRepr};

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum InputEvent {
    MouseMove { dx: i32, dy: i32 },

    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },

    MouseScroll {},

    KeyDown { key: KeyCode },
    KeyRepeat { key: KeyCode },
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

/// Define a bidirectional injective conversion.
///
/// Given a set A and a set B. This macro takes definition of `A -> B`, in which it will generates a function of `A -> B` and `B -> Option A`.
///
/// Its injective property would not be typechecked.
macro_rules! def_conversion {
    ($l_ty:ty, $r_ty:ty, { $($l_var:ident => $r_var:ident,)* }) => {
        impl Into<$r_ty> for $l_ty {
            fn into(self) -> $r_ty {
                use $l_ty::*;
                use $r_ty::*;
                match self {
                    $($l_var => $r_var,)*
                }
            }
        }

        paste::paste! {
            impl $l_ty {
                pub fn [<from_$r_ty:lower>](x: $r_ty) -> Option<Self> {
                    use $r_ty::*;
                    use $l_ty::*;
                    match x {
                        $($r_var => Some($l_var),)*
                        _ => None,
                    }
                }
            }
        }
    };

    (
        $l_ty:ty,
        $r_ty:ty,
        {
            $($l_var:ident = $r_var:expr,)*
        }
    ) => {
        impl Into<$r_ty> for $l_ty {
            fn into(self) -> $r_ty {
                use $l_ty::*;
                match self {
                    $($l_var => $r_var,)*
                }
            }
        }

        paste::paste! {
            impl $l_ty {
                pub fn [<from_$r_ty:lower>](x: $r_ty) -> Option<Self> {
                    use $l_ty::*;
                    match x {
                        $(x if x == $r_var => Some($l_var),)*
                        _ => None,
                    }
                }
            }
        }
    };
}

#[cfg(target_os = "linux")]
pub mod linux {
    use super::*;
    use evdev_rs::enums::EV_KEY;

    def_conversion!(KeyCode, EV_KEY, {
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

        Tab => KEY_TAB,
        CapsLock => KEY_CAPSLOCK,

        LeftShift => KEY_LEFTSHIFT,
        RightShift => KEY_RIGHTSHIFT,

        LeftCtrl => KEY_LEFTCTRL,
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
    });

    def_conversion!(MouseButton, EV_KEY, {
        Left => BTN_LEFT,
        Right => BTN_RIGHT,
        Middle => BTN_MIDDLE,
        Mouse4 => BTN_4,
        Mouse5 => BTN_5,
    });
}

#[cfg(target_os = "windows")]
pub mod windows {
    use super::*;
    use ::windows::Win32::UI::Input::KeyboardAndMouse::*;
    use macross::newtype;

    newtype! {
        #[derive(PartialEq, Debug)]
        pub VirtualKey = u16;
    }

    // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
    def_conversion!(KeyCode, VirtualKey, {
        Escape = VK_ESCAPE.0.into(),

        F1 = VK_F1.0.into(),
        F2 = VK_F2.0.into(),
        F3 = VK_F3.0.into(),
        F4 = VK_F4.0.into(),
        F5 = VK_F5.0.into(),
        F6 = VK_F6.0.into(),
        F7 = VK_F7.0.into(),
        F8 = VK_F8.0.into(),
        F9 = VK_F9.0.into(),
        F10 = VK_F10.0.into(),
        F11 = VK_F11.0.into(),
        F12 = VK_F12.0.into(),

        PrintScreen = VK_SNAPSHOT.0.into(),
        ScrollLock = VK_SCROLL.0.into(),
        PauseBreak = VK_PAUSE.0.into(),

        Grave = VK_OEM_3.0.into(),

        D1 = 0x31.into(),
        D2 = 0x32.into(),
        D3 = 0x33.into(),
        D4 = 0x34.into(),
        D5 = 0x35.into(),
        D6 = 0x36.into(),
        D7 = 0x37.into(),
        D8 = 0x38.into(),
        D9 = 0x39.into(),
        D0 = 0x30.into(),

        Minus = VK_OEM_MINUS.0.into(),
        Equal = VK_OEM_PLUS.0.into(),

        A = 0x41.into(),
        B = 0x42.into(),
        C = 0x43.into(),
        D = 0x44.into(),
        E = 0x45.into(),
        F = 0x46.into(),
        G = 0x47.into(),
        H = 0x48.into(),
        I = 0x49.into(),
        J = 0x4A.into(),
        K = 0x4B.into(),
        L = 0x4C.into(),
        M = 0x4D.into(),
        N = 0x4E.into(),
        O = 0x4F.into(),
        P = 0x50.into(),
        Q = 0x51.into(),
        R = 0x52.into(),
        S = 0x53.into(),
        T = 0x54.into(),
        U = 0x55.into(),
        V = 0x56.into(),
        W = 0x57.into(),
        X = 0x58.into(),
        Y = 0x59.into(),
        Z = 0x5A.into(),

        LeftBrace = VK_OEM_4.0.into(),
        RightBrace = VK_OEM_6.0.into(),

        SemiColon = VK_OEM_1.0.into(),
        Apostrophe = VK_OEM_7.0.into(),

        Comma = VK_OEM_COMMA.0.into(),
        Dot = VK_OEM_PERIOD.0.into(),
        Slash = VK_OEM_2.0.into(),

        Backspace = VK_BACK.0.into(),
        BackSlash = VK_OEM_5.0.into(),
        Enter = VK_RETURN.0.into(),

        Space = VK_SPACE.0.into(),

        Tab = VK_TAB.0.into(),
        CapsLock = VK_CAPITAL.0.into(),

        LeftShift = VK_LSHIFT.0.into(),
        RightShift = VK_RSHIFT.0.into(),

        LeftCtrl = VK_LCONTROL.0.into(),
        RightCtrl = VK_RCONTROL.0.into(),

        LeftAlt = VK_LMENU.0.into(),
        RightAlt = VK_RMENU.0.into(),

        LeftMeta = VK_LWIN.0.into(),
        RightMeta = VK_RWIN.0.into(),

        Insert = VK_INSERT.0.into(),
        Delete = VK_DELETE.0.into(),

        Home = VK_HOME.0.into(),
        End = VK_END.0.into(),

        PageUp = VK_PRIOR.0.into(),
        PageDown = VK_NEXT.0.into(),

        Up = VK_UP.0.into(),
        Left = VK_LEFT.0.into(),
        Down = VK_DOWN.0.into(),
        Right = VK_RIGHT.0.into(),
    });

    newtype! {
        #[derive(PartialEq, Debug)]
        pub ScanCode = u16;
    }

    // https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/translate.pdf column `PS/2 Set 1 Make`
    def_conversion!(KeyCode, ScanCode, {
        Escape = 0xd1.into(),

        F1 = 0x3b.into(),
        F2 = 0xd1.into(),
        F3 = 0xd1.into(),
        F4 = 0xd1.into(),
        F5 = 0xd1.into(),
        F6 = 0xd1.into(),
        F7 = 0xd1.into(),
        F8 = 0xd1.into(),
        F9 = 0xd1.into(),
        F10 = 0xd1.into(),
        F11 = 0xd1.into(),
        F12 = 0xd1.into(),

        PrintScreen = 0xd1.into(),
        ScrollLock = 0xd1.into(),
        PauseBreak = 0xd1.into(),

        Grave = 0xd1.into(),

        D1 = 0xd1.into(),
        D2 = 0xd1.into(),
        D3 = 0xd1.into(),
        D4 = 0xd1.into(),
        D5 = 0xd1.into(),
        D6 = 0xd1.into(),
        D7 = 0xd1.into(),
        D8 = 0xd1.into(),
        D9 = 0xd1.into(),
        D0 = 0xd1.into(),

        Minus = 0xd1.into(),
        Equal = 0xd1.into(),

        A = 0xd1.into(),
        B = 0xd1.into(),
        C = 0x2e.into(),
        D = 0xd1.into(),
        E = 0xd1.into(),
        F = 0xd1.into(),
        G = 0xd1.into(),
        H = 0xd1.into(),
        I = 0xd1.into(),
        J = 0xd1.into(),
        K = 0xd1.into(),
        L = 0xd1.into(),
        M = 0xd1.into(),
        N = 0xd1.into(),
        O = 0xd1.into(),
        P = 0xd1.into(),
        Q = 0xd1.into(),
        R = 0xd1.into(),
        S = 0xd1.into(),
        T = 0xd1.into(),
        U = 0xd1.into(),
        V = 0xd1.into(),
        W = 0xd1.into(),
        X = 0xd1.into(),
        Y = 0xd1.into(),
        Z = 0xd1.into(),

        LeftBrace = 0xd1.into(),
        RightBrace = 0xd1.into(),

        SemiColon = 0xd1.into(),
        Apostrophe = 0xd1.into(),

        Comma = 0xd1.into(),
        Dot = 0xd1.into(),
        Slash = 0xd1.into(),

        Backspace = 0xd1.into(),
        BackSlash = 0xd1.into(),
        Enter = 0xd1.into(),

        Space = 0xd1.into(),

        Tab = 0xd1.into(),
        CapsLock = 0xd1.into(),

        LeftShift = 0x2a.into(),
        RightShift = 0x36.into(),

        LeftCtrl = 0x1d.into(),
        RightCtrl = 0xd1.into(),

        LeftAlt = 0xd1.into(),
        RightAlt = 0xd1.into(),

        LeftMeta = 0xd1.into(),
        RightMeta = 0xd1.into(),

        Insert = 0xd1.into(),
        Delete = 0xd1.into(),

        Home = 0xd1.into(),
        End = 0xd1.into(),

        PageUp = 0xd1.into(),
        PageDown = 0xd1.into(),

        Up = 0xd1.into(),
        Left = 0xd1.into(),
        Down = 0xd1.into(),
        Right = 0xd1.into(),
    });
}
