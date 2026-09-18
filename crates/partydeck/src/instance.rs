use crate::monitor::Monitor;
use crate::app::PartyConfig;
use crate::profiles::GUEST_NAMES;

#[derive(Clone)]
pub struct Instance {
    pub devices: Vec<usize>,
    // Explicit XInput slot for this player's proxy pad; None derives it from devices.
    pub pad_slot: Option<u32>,
    pub profname: String,
    pub profselection: usize,
    pub monitor: usize,
    pub width: u32,
    pub height: u32,
}

pub fn set_instance_resolutions_from_layout(
    instances: &mut Vec<Instance>,
    primary_monitor: &Monitor,
    layout: &partydeck_comp_proto::layout::Layout,
    cfg: &PartyConfig,
) {
    let rects = layout.resolve(primary_monitor.width(), primary_monitor.height());
    for (instance, rect) in instances.iter_mut().zip(rects) {
        let (mut w, mut h) = (rect.w.max(1) as u32, rect.h.max(1) as u32);
        if h < 600 && cfg.gamescope_fix_lowres {
            let ratio = w as f32 / h as f32;
            h = 600;
            w = (h as f32 * ratio) as u32;
        }
        instance.width = w;
        instance.height = h;
    }
}

pub fn set_instance_names(instances: &mut Vec<Instance>, profiles: &[String]) {
    let mut guests = GUEST_NAMES.to_vec();

    for instance in instances {
        if instance.profselection == 0 {
            let i = fastrand::usize(..guests.len());
            instance.profname = format!(".{}", guests[i]);
            guests.swap_remove(i);
        } else {
            instance.profname = profiles[instance.profselection].to_owned();
        }
    }
}
