use crate::input_event::{KeyCode, MouseButton};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LocalInputEvent {
    MousePosition(MousePosition),

    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },
    MouseScroll {},

    KeyDown { key: KeyCode },
    KeyUp { key: KeyCode },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

impl MousePosition {
    pub fn delta_to(&self, other: Self) -> (i32, i32) {
        todo!()
    }
}
