use crate::protocol::{InputEvent, KeyCode, MouseButton, MouseScrollDirection};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LocalInputEvent {
    MousePosition(MousePosition),
    MouseMove(MouseMovement),

    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },
    MouseScroll { direction: MouseScrollDirection },

    KeyDown { key: KeyCode },
    KeyRepeat { key: KeyCode },
    KeyUp { key: KeyCode },
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct MouseMovement {
    pub dx: i16,
    pub dy: i16,
}

impl From<(i16, i16)> for MouseMovement {
    fn from((dx, dy): (i16, i16)) -> Self {
        Self { dx, dy }
    }
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct MousePosition {
    pub x: i16,
    pub y: i16,
}

impl From<(i16, i16)> for MousePosition {
    fn from((x, y): (i16, i16)) -> Self {
        Self { x, y }
    }
}

impl MousePosition {
    pub fn delta_to(&self, other: &Self) -> MouseMovement {
        let MousePosition { x: x1, y: y1 } = *self;
        let MousePosition { x: x2, y: y2 } = *other;
        let dx = x2 - x1;
        let dy = y2 - y1;
        MouseMovement { dx, dy }
    }
}

impl LocalInputEvent {
    /// Converts local input event into protocol input event.
    pub fn into_input_event(self) -> Option<InputEvent> {
        match self {
            LocalInputEvent::MouseMove(MouseMovement { dx, dy }) => {
                InputEvent::MouseMove { dx, dy }.into()
            }
            LocalInputEvent::MouseButtonDown { button } => {
                InputEvent::MouseButtonDown { button }.into()
            }
            LocalInputEvent::MouseButtonUp { button } => {
                InputEvent::MouseButtonUp { button }.into()
            }
            LocalInputEvent::MouseScroll { direction } => {
                InputEvent::MouseScroll { direction }.into()
            }
            LocalInputEvent::KeyDown { key } => InputEvent::KeyDown { key }.into(),
            LocalInputEvent::KeyRepeat { key } => InputEvent::KeyRepeat { key }.into(),
            LocalInputEvent::KeyUp { key } => InputEvent::KeyUp { key }.into(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_to() {
        let original = MousePosition { x: 1, y: 1 };
        assert_eq!(original.delta_to(&original), (0, 0).into());
        assert_eq!(
            original.delta_to(&MousePosition { x: 1, y: -1 }),
            (0, -2).into()
        );
        assert_eq!(
            original.delta_to(&MousePosition { x: -1, y: -1 }),
            (-2, -2).into()
        );
        assert_eq!(
            original.delta_to(&MousePosition { x: -1, y: 1 }),
            (-2, 0).into()
        );

        let original = MousePosition { x: 1, y: -1 };
        assert_eq!(original.delta_to(&original), (0, 0).into());
        assert_eq!(
            original.delta_to(&MousePosition { x: -1, y: -1 }),
            (-2, 0).into()
        );
        assert_eq!(
            original.delta_to(&MousePosition { x: -1, y: 1 }),
            (-2, 2).into()
        );
        assert_eq!(
            original.delta_to(&MousePosition { x: 1, y: 1 }),
            (0, 2).into()
        );
    }
}
