//! Quartz event translation, independent of OS posting so it can be replayed in tests.
use crossdesk_core::Point;
use std::collections::BTreeSet;

#[derive(Default)]
pub(crate) struct ModifierState {
    pressed: BTreeSet<u16>,
}

impl ModifierState {
    pub fn seed(&mut self, code: u16) {
        self.pressed.insert(code);
    }

    pub fn changed(&mut self, code: u16, flags: u64) -> Option<bool> {
        // Public CGEventFlagMask* bits are device independent. The optional
        // NX device bits distinguish left/right, but are absent on some input
        // paths. Do not treat their absence as "Control is up".
        let (aggregate, side, pair) = match code {
            59 => (1 << 18, 0x1, 0x2001),
            62 => (1 << 18, 0x2000, 0x2001),
            56 => (1 << 17, 0x2, 0x6),
            60 => (1 << 17, 0x4, 0x6),
            58 => (1 << 19, 0x20, 0x60),
            61 => (1 << 19, 0x40, 0x60),
            55 => (1 << 20, 0x8, 0x18),
            54 => (1 << 20, 0x10, 0x18),
            57 => (1 << 16, 1 << 16, 1 << 16),
            _ => return None,
        };
        let down = flags & aggregate != 0
            && if flags & pair != 0 {
                flags & side != 0
            } else {
                !self.pressed.contains(&code)
            };
        if down {
            self.pressed.insert(code);
        } else {
            self.pressed.remove(&code);
        }
        Some(down)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MouseEvent {
    pub kind: u32,
    pub button: u32,
    pub position: Point,
    pub delta: Point,
    pub number: i64,
    pub pressed: bool,
}

#[derive(Clone, Default)]
pub(crate) struct MouseState {
    position: Option<Point>,
    buttons: [bool; 5],
    numbers: [i64; 5],
    next_number: i64,
}

impl MouseState {
    pub fn motion(&mut self, position: Point) -> MouseEvent {
        let previous = self.position.unwrap_or(position);
        self.position = Some(position);
        let held = self.buttons.iter().position(|down| *down);
        let button = held.unwrap_or(0);
        MouseEvent {
            kind: match held {
                Some(0) => 6, // kCGEventLeftMouseDragged
                Some(1) => 7, // kCGEventRightMouseDragged
                Some(_) => 27,
                None => 5,
            },
            button: button as u32,
            position,
            delta: Point {
                x: position.x - previous.x,
                y: position.y - previous.y,
            },
            number: if held.is_some() {
                self.numbers[button]
            } else {
                0
            },
            pressed: held.is_some(),
        }
    }

    pub fn button(&mut self, button: u8, down: bool, position: Point) -> Option<MouseEvent> {
        if !(1..=5).contains(&button) {
            return None;
        }
        let index = (button - 1) as usize;
        if down && !self.buttons[index] {
            self.next_number = self.next_number.saturating_add(1);
            self.numbers[index] = self.next_number;
        }
        self.buttons[index] = down;
        self.position = Some(position);
        Some(MouseEvent {
            kind: match (index, down) {
                (0, true) => 1,
                (0, false) => 2,
                (1, true) => 3,
                (1, false) => 4,
                (_, true) => 25,
                (_, false) => 26,
            },
            button: index as u32,
            position,
            delta: Point::default(),
            number: self.numbers[index],
            pressed: down,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_modifier_flags_work_without_device_specific_bits() {
        for (left, right, flags) in [
            (59, 62, 1 << 18),
            (56, 60, 1 << 17),
            (58, 61, 1 << 19),
            (55, 54, 1 << 20),
        ] {
            let mut state = ModifierState::default();
            assert_eq!(state.changed(left, flags), Some(true));
            assert_eq!(state.changed(right, flags), Some(true));
            assert_eq!(state.changed(left, flags), Some(false));
            assert_eq!(state.changed(right, 0), Some(false));
            assert!(state.pressed.is_empty());
        }
    }

    #[test]
    fn side_flags_and_already_held_keys_do_not_stick() {
        let mut state = ModifierState::default();
        state.seed(59);
        assert_eq!(state.changed(62, (1 << 18) | 0x2001), Some(true));
        assert_eq!(state.changed(59, (1 << 18) | 0x2000), Some(false));
        assert_eq!(state.changed(62, 0), Some(false));
        state.seed(59);
        assert_eq!(state.changed(59, 0), Some(false));
    }

    #[test]
    fn move_press_drag_release_uses_matching_event_number_and_live_release_position() {
        for (button, drag) in [(1, 6), (2, 7), (3, 27), (4, 27), (5, 27)] {
            let mut state = MouseState::default();
            let p = Point {
                x: -400.5,
                y: 200.25,
            };
            assert_eq!(state.motion(p).kind, 5);
            let down = state.button(button, true, p).unwrap();
            assert_eq!(down.position, p);
            assert!(down.number > 0);
            for _ in 0..3 {
                let moved = state.motion(Point {
                    x: -395.5,
                    y: 220.25,
                });
                assert_eq!((moved.kind, moved.button), (drag, (button - 1) as u32));
                assert_eq!(moved.number, down.number);
            }
            // A target's own mouse may have taken over before safety release.
            let local = Point { x: 100., y: 50. };
            let up = state.button(button, false, local).unwrap();
            assert_eq!(up.position, local);
            assert_eq!(up.number, down.number);
            assert!(!up.pressed);
            assert_eq!(state.motion(p).kind, 5);
        }
    }

    #[test]
    fn multiple_buttons_and_safety_releases_restore_plain_motion() {
        let mut state = MouseState::default();
        state.button(2, true, Point::default());
        state.button(1, true, Point::default());
        assert_eq!(state.motion(Point::default()).kind, 6);
        state.button(1, false, Point::default());
        assert_eq!(state.motion(Point::default()).kind, 7);
        state.button(2, false, Point::default());
        assert_eq!(state.motion(Point::default()).kind, 5);
        assert!(state.button(0, true, Point::default()).is_none());
        assert!(state.button(6, true, Point::default()).is_none());
    }

    #[test]
    fn drag_delta_preserves_fractional_and_negative_coordinates() {
        let mut state = MouseState::default();
        state.button(1, true, Point { x: -10., y: -20. });
        let event = state.motion(Point { x: -8.5, y: -23. });
        assert_eq!(event.delta, Point { x: 1.5, y: -3. });
        assert!(event.pressed);
    }
}
