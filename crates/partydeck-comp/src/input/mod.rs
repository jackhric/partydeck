mod pointer;

use smithay::backend::input::{Event, InputBackend, InputEvent, KeyboardKeyEvent};
use smithay::input::keyboard::FilterResult;
use smithay::utils::SERIAL_COUNTER;

use crate::state::CompState;

impl CompState {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => self.forward_key(event),
            InputEvent::PointerMotionAbsolute { event, .. } => {
                pointer::motion_absolute::<I>(self, event)
            }
            InputEvent::PointerButton { event, .. } => pointer::button::<I>(self, event),
            InputEvent::PointerAxis { event, .. } => pointer::axis::<I>(self, event),
            _ => {}
        }
    }

    fn forward_key<E: KeyboardKeyEvent<I>, I: InputBackend>(&mut self, event: E) {
        let Some(keyboard) = self.seat.get_keyboard() else {
            return;
        };
        keyboard.input::<(), _>(
            self,
            event.key_code(),
            event.state(),
            SERIAL_COUNTER.next_serial(),
            Event::time_msec(&event),
            |_, _, _| FilterResult::Forward,
        );
    }
}
