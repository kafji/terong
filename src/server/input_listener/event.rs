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
    pub fn delta_to(&self, other: &Self) -> (i32, i32) {
        let MousePosition { x: x1, y: y1 } = *self;
        let MousePosition { x: x2, y: y2 } = *other;
        let x = x2 - x1;
        let y = y2 - y1;
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_to() {
        assert_eq!(
            MousePosition { x: 1, y: 1 }.delta_to(&MousePosition { x: -1, y: -1 }),
            (-2, -2)
        );
    }
}
