use partydeck_comp_proto::layout::PixelRect;
use partydeck_comp_proto::state::{Size, SlotState, State};

use crate::state::{CompState, SlotInfo};

pub fn build(state: &CompState) -> State {
    let size = state.output_size();
    let live: Vec<bool> = state.slot_windows.iter().map(Option::is_some).collect();
    State::new(
        Size {
            w: size.w,
            h: size.h,
        },
        state.layout.focus,
        state.border,
        slot_states(&state.slot_rects(), &live, &state.slot_info),
    )
}

pub fn slot_states(
    rects: &[PixelRect],
    live: &[bool],
    info: &[Option<SlotInfo>],
) -> Vec<SlotState> {
    rects
        .iter()
        .enumerate()
        .map(|(i, &rect)| {
            let info = info.get(i).and_then(Option::as_ref);
            SlotState {
                rect,
                live: live.get(i).copied().unwrap_or(false),
                status: info.map(|s| s.status),
                label: info.and_then(|s| s.label.clone()),
                avatar: info.and_then(|s| s.avatar.clone()),
                logo: info.and_then(|s| s.logo.clone()),
                controller_disconnected: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use partydeck_comp_proto::presets;
    use partydeck_comp_proto::state::SlotStatus;

    #[test]
    fn slot_states_merge_rects_liveness_and_info() {
        let rects = presets::quadrants(2).resolve(1280, 800);
        let info = vec![
            Some(SlotInfo {
                status: SlotStatus::Loading,
                label: Some("p1".into()),
                avatar: None,
                logo: Some("bG9nbw==".into()),
            }),
            None,
        ];
        let slots = slot_states(&rects, &[true, false], &info);
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].rect, rects[0]);
        assert!(slots[0].live);
        assert_eq!(slots[0].status, Some(SlotStatus::Loading));
        assert_eq!(slots[0].label.as_deref(), Some("p1"));
        assert_eq!(slots[0].logo.as_deref(), Some("bG9nbw=="));
        assert!(!slots[1].live);
        assert_eq!(slots[1].status, None);
        assert!(slots.iter().all(|s| !s.controller_disconnected));
    }

    #[test]
    fn short_side_tables_default_to_dead_and_empty() {
        let rects = presets::quadrants(3).resolve(1280, 800);
        let slots = slot_states(&rects, &[true], &[]);
        assert_eq!(slots.len(), 3);
        assert_eq!(
            slots.iter().map(|s| s.live).collect::<Vec<_>>(),
            [true, false, false]
        );
        assert!(
            slots
                .iter()
                .all(|s| s.status.is_none() && s.label.is_none())
        );
    }

    #[test]
    fn document_carries_protocol_version() {
        let doc = State::new(Size { w: 1, h: 1 }, 0, Default::default(), vec![]);
        assert_eq!(doc.proto, partydeck_comp_proto::PROTOCOL_VERSION);
    }
}
