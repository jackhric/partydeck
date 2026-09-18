use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, PointerAxisEvent,
    PointerButtonEvent,
};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::SERIAL_COUNTER;

use crate::state::CompState;

pub fn motion_absolute<I: InputBackend>(
    state: &mut CompState,
    event: I::PointerMotionAbsoluteEvent,
) {
    let Some(output_geo) = state
        .space
        .outputs()
        .next()
        .and_then(|o| state.space.output_geometry(o))
    else {
        return;
    };
    let Some(pointer) = state.seat.get_pointer() else {
        return;
    };
    let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
    let under = state.surface_under(pos);
    pointer.motion(
        state,
        under,
        &MotionEvent {
            location: pos,
            serial: SERIAL_COUNTER.next_serial(),
            time: event.time_msec(),
        },
    );
    pointer.frame(state);
}

pub fn button<I: InputBackend>(state: &mut CompState, event: I::PointerButtonEvent) {
    let (Some(pointer), Some(keyboard)) = (state.seat.get_pointer(), state.seat.get_keyboard())
    else {
        return;
    };
    let serial = SERIAL_COUNTER.next_serial();
    let button_state = event.state();

    if button_state == ButtonState::Pressed && !pointer.is_grabbed() {
        let under = state
            .space
            .element_under(pointer.current_location())
            .map(|(w, _)| w.clone());
        match under {
            Some(window) => {
                state.space.raise_element(&window, true);
                let surface = window.toplevel().map(|t| t.wl_surface().clone());
                keyboard.set_focus(state, surface, serial);
                for window in state.space.elements() {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_pending_configure();
                    }
                }
            }
            None => {
                for window in state.space.elements() {
                    window.set_activated(false);
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_pending_configure();
                    }
                }
                keyboard.set_focus(state, Option::<WlSurface>::None, serial);
            }
        }
    }

    pointer.button(
        state,
        &ButtonEvent {
            button: event.button_code(),
            state: button_state,
            serial,
            time: event.time_msec(),
        },
    );
    pointer.frame(state);
}

pub fn axis<I: InputBackend>(state: &mut CompState, event: I::PointerAxisEvent) {
    let Some(pointer) = state.seat.get_pointer() else {
        return;
    };
    let source = event.source();
    let amount = |axis: Axis| {
        event
            .amount(axis)
            .unwrap_or_else(|| event.amount_v120(axis).unwrap_or(0.0) * 15.0 / 120.0)
    };

    let mut frame = AxisFrame::new(event.time_msec()).source(source);
    for axis in [Axis::Horizontal, Axis::Vertical] {
        let value = amount(axis);
        if value != 0.0 {
            frame = frame.value(axis, value);
            if let Some(discrete) = event.amount_v120(axis) {
                frame = frame.v120(axis, discrete as i32);
            }
        }
        if source == AxisSource::Finger && event.amount(axis) == Some(0.0) {
            frame = frame.stop(axis);
        }
    }

    pointer.axis(state, frame);
    pointer.frame(state);
}
