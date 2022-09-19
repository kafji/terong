use crate::protocol::{KeyCode, MouseButton};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LocalInputEvent {
    MousePosition(MousePosition),

    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },
    MouseScroll {},

    KeyDown { key: KeyCode },
    KeyRepeat { key: KeyCode },
    KeyUp { key: KeyCode },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

impl MousePosition {
    pub fn delta_to(&self, other: &Self) -> (i32 /* dx */, i32 /* dy */) {
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
        let original = MousePosition { x: 1, y: 1 };
        assert_eq!(original.delta_to(&original), (0, 0));
        assert_eq!(original.delta_to(&MousePosition { x: 1, y: -1 }), (0, -2));
        assert_eq!(original.delta_to(&MousePosition { x: -1, y: -1 }), (-2, -2));
        assert_eq!(original.delta_to(&MousePosition { x: -1, y: 1 }), (-2, 0));

        let original = MousePosition { x: 1, y: -1 };
        assert_eq!(original.delta_to(&original), (0, 0));
        assert_eq!(original.delta_to(&MousePosition { x: -1, y: -1 }), (-2, 0));
        assert_eq!(original.delta_to(&MousePosition { x: -1, y: 1 }), (-2, 2));
        assert_eq!(original.delta_to(&MousePosition { x: 1, y: 1 }), (0, 2));
    }
}
