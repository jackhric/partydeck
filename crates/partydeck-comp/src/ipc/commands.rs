use partydeck_comp_proto::ipc::{Command, Response};

use super::state as state_doc;
use crate::state::{CompState, SlotInfo};

pub fn apply(state: &mut CompState, cmd: Command) -> Response {
    match cmd {
        Command::GetState => Response::State(state_doc::build(state)),
        Command::Quit => {
            state.loop_signal.stop();
            Response::Ok
        }
        Command::SetSlotStatus {
            slot,
            status,
            label,
            avatar,
            logo,
        } => {
            let Some(entry) = state.slot_info.get_mut(slot) else {
                return Response::Err(format!("slot {slot} out of range"));
            };
            *entry = Some(SlotInfo {
                status,
                label,
                avatar,
                logo,
            });
            Response::Ok
        }
    }
}
