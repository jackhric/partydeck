use evdev::{AbsoluteAxisCode, EventSummary, KeyCode};

use super::InputDevice;

/// Buttons the GUI reacts to when navigating with a pad, keyboard or mouse.
pub enum PadButton {
    Left,
    Right,
    Up,
    Down,
    ABtn,
    BBtn,
    XBtn,
    YBtn,
    StartBtn,
    SelectBtn,

    AKey,
    XKey,
    ZKey,

    RightClick,
}

impl InputDevice {
    /// Drains pending events and returns the last recognised press, if any.
    pub fn poll(&mut self) -> Option<PadButton> {
        let mut btn: Option<PadButton> = None;
        let Ok(events) = self.dev.fetch_events() else {
            return None;
        };
        for event in events {
            let summary = event.destructure();
            match summary {
                EventSummary::Key(_, _, 1) => self.has_button_held = true,
                EventSummary::Key(_, _, 0) => self.has_button_held = false,
                _ => {}
            }
            if let Some(pressed) = button_for(&summary) {
                btn = Some(pressed);
            }
        }
        btn
    }
}

fn button_for(summary: &EventSummary) -> Option<PadButton> {
    Some(match summary {
        EventSummary::Key(_, KeyCode::BTN_SOUTH, 1) => PadButton::ABtn,
        EventSummary::Key(_, KeyCode::BTN_EAST, 1) => PadButton::BBtn,
        EventSummary::Key(_, KeyCode::BTN_NORTH, 1) => PadButton::XBtn,
        EventSummary::Key(_, KeyCode::BTN_WEST, 1) => PadButton::YBtn,
        EventSummary::Key(_, KeyCode::BTN_START, 1) => PadButton::StartBtn,
        EventSummary::Key(_, KeyCode::BTN_SELECT, 1) => PadButton::SelectBtn,
        EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0X, -1) => PadButton::Left,
        EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0X, 1) => PadButton::Right,
        EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0Y, -1) => PadButton::Up,
        EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_HAT0Y, 1) => PadButton::Down,
        EventSummary::Key(_, KeyCode::KEY_A, 1) => PadButton::AKey,
        EventSummary::Key(_, KeyCode::KEY_X, 1) => PadButton::XKey,
        EventSummary::Key(_, KeyCode::KEY_Z, 1) => PadButton::ZKey,
        EventSummary::Key(_, KeyCode::BTN_RIGHT, 1) => PadButton::RightClick,
        _ => return None,
    })
}
