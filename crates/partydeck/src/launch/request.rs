use std::collections::{HashMap, HashSet};
use std::path::Path;

use partydeck_comp_proto::layout::Layout;
use partydeck_comp_proto::presets;
use serde::Deserialize;

use crate::config::{PadFilterType, PartyConfig, load_cfg};
use crate::error::Result;
use crate::handler::Handler;
use crate::input::{DeviceInfo, DeviceType, scan_input_devices};
use crate::instance::{Instance, ProfileChoice};
use crate::monitor::Monitor;
use crate::profile::{GUEST_NAMES, guest_dir_name, scan_profiles};

const MIN_INSTANCE_HEIGHT: u32 = 600;

/// One player in the `launch --players` JSON file. Array order is split order.
#[derive(Deserialize, Debug, Clone)]
pub struct PlayerSpec {
    pub profile: String,
    /// Steam Input XInput slot (== nXInputIndex == "Microsoft X-Box 360 pad N").
    pub xinput: u32,
}

pub struct LaunchRequest {
    pub handler: Handler,
    pub instances: Vec<Instance>,
    pub devices: Vec<DeviceInfo>,
    pub cfg: PartyConfig,
    pub monitor: Monitor,
    pub layout: Layout,
}

impl LaunchRequest {
    /// Resolves guest names; `instances` must already carry their devices.
    pub fn new(
        handler: Handler,
        mut instances: Vec<Instance>,
        devices: Vec<DeviceInfo>,
        cfg: PartyConfig,
        monitor: Monitor,
        layout: Layout,
    ) -> Self {
        assign_profile_names(&mut instances);
        LaunchRequest {
            handler,
            instances,
            devices,
            cfg,
            monitor,
            layout,
        }
    }

    /// Headless request: each player is bound to the Steam Input pad in its
    /// XInput slot. The lobby joins through Steam Input, so only those pads
    /// are scanned.
    pub fn from_player_specs(
        handler: Handler,
        players: &[PlayerSpec],
        layout_file: Option<&Path>,
    ) -> Result<Self> {
        if players.is_empty() {
            return Err("no players".into());
        }
        let mut cfg = load_cfg();
        cfg.pad_filter_type = PadFilterType::OnlySteamInput;
        let proxy_mode = cfg.proxy_gamepads && !handler.enable_hidraw;

        let devices: Vec<DeviceInfo> = scan_input_devices(&cfg.pad_filter_type)
            .iter()
            .map(|d| d.info())
            .collect();
        let instances = instances_from_specs(players, &scan_profiles(false), &devices, proxy_mode)?;

        let layout = match layout_file {
            Some(path) => layout_from_file(path, players.len())?,
            None => default_layout(&cfg.layout_preset, players.len()),
        };
        let monitor = Monitor::new("test", 1920, 1080);
        Ok(LaunchRequest::new(
            handler, instances, devices, cfg, monitor, layout,
        ))
    }
}

/// Fails fast on bad specs: a missing profile would otherwise surface as a
/// cryptic bwrap bind error mid-launch, and a duplicated slot would feed one
/// pad to two instances. In proxy mode a pad that is not plugged in yet is
/// fine; the router attaches when it appears.
pub fn instances_from_specs(
    players: &[PlayerSpec],
    profiles: &[String],
    devices: &[DeviceInfo],
    proxy_mode: bool,
) -> Result<Vec<Instance>> {
    let mut seen_slots = HashSet::new();
    for p in players {
        if !profiles.contains(&p.profile) {
            return Err(format!("no profile named {:?}", p.profile).into());
        }
        if !seen_slots.insert(p.xinput) {
            return Err(format!(
                "XInput slot {} is assigned to more than one player",
                p.xinput
            )
            .into());
        }
    }

    let mut slot_to_index: HashMap<u32, usize> = HashMap::new();
    for (i, d) in devices.iter().enumerate() {
        if d.enabled
            && d.device_type == DeviceType::Gamepad
            && let Some(slot) = d.xinput_slot
        {
            slot_to_index.entry(slot).or_insert(i);
        }
    }

    players
        .iter()
        .map(|p| {
            let devices = match slot_to_index.get(&p.xinput) {
                Some(&dev_index) => vec![dev_index],
                None if proxy_mode => {
                    eprintln!(
                        "[partydeck] launch: no Steam Input pad for XInput slot {} yet (player {:?}); proxy will attach when it appears",
                        p.xinput, p.profile
                    );
                    Vec::new()
                }
                None => {
                    return Err(format!(
                        "no Steam Input pad for XInput slot {} (player {:?})",
                        p.xinput, p.profile
                    )
                    .into());
                }
            };
            Ok(Instance {
                pad_slot: Some(p.xinput),
                profile: ProfileChoice::Named(p.profile.clone()),
                ..Instance::new(devices)
            })
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum LayoutSpec {
    Preset { preset: String },
    Full(Layout),
}

/// Reads `{"preset": "grid"}` or a full layout document and validates it.
pub fn layout_from_file(path: &Path, players: usize) -> Result<Layout> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read layout file {}: {e}", path.display()))?;
    let spec: LayoutSpec =
        serde_json::from_str(&text).map_err(|e| format!("invalid layout JSON: {e}"))?;
    let layout = match spec {
        LayoutSpec::Full(layout) => layout,
        LayoutSpec::Preset { preset } => presets::by_name(&preset, players)
            .ok_or_else(|| format!("unknown layout preset {preset:?}"))?,
    };
    layout.validate(players)?;
    Ok(layout)
}

/// The configured preset, or quadrants when the name is unknown.
pub fn default_layout(preset: &str, players: usize) -> Layout {
    presets::by_name(preset, players).unwrap_or_else(|| presets::quadrants(players))
}

/// Guests get a random unused name from GUEST_NAMES with the guest '.' prefix.
pub fn assign_profile_names(instances: &mut [Instance]) {
    let mut guests = GUEST_NAMES.to_vec();
    for instance in instances {
        instance.profname = match &instance.profile {
            ProfileChoice::Named(name) => name.clone(),
            ProfileChoice::Guest => {
                if guests.is_empty() {
                    guests = GUEST_NAMES.to_vec();
                }
                let i = fastrand::usize(..guests.len());
                guest_dir_name(guests.swap_remove(i))
            }
        };
    }
}

pub fn set_instance_resolutions_from_layout(
    instances: &mut [Instance],
    monitor: &Monitor,
    layout: &Layout,
    cfg: &PartyConfig,
) {
    let rects = layout.resolve(monitor.width(), monitor.height());
    for (instance, rect) in instances.iter_mut().zip(rects) {
        let (mut w, mut h) = (rect.w.max(1) as u32, rect.h.max(1) as u32);
        // Many games glitch or crash below 600p; upscale keeping the aspect ratio.
        if h < MIN_INSTANCE_HEIGHT && cfg.gamescope_fix_lowres {
            let ratio = w as f32 / h as f32;
            h = MIN_INSTANCE_HEIGHT;
            w = (h as f32 * ratio) as u32;
        }
        instance.width = w;
        instance.height = h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(slot: Option<u32>, enabled: bool) -> DeviceInfo {
        DeviceInfo {
            path: format!("/dev/input/event{}", slot.unwrap_or(99)),
            hidraw_paths: Vec::new(),
            enabled,
            device_type: DeviceType::Gamepad,
            xinput_slot: slot,
        }
    }

    fn spec(profile: &str, xinput: u32) -> PlayerSpec {
        PlayerSpec {
            profile: profile.to_string(),
            xinput,
        }
    }

    #[test]
    fn unknown_preset_falls_back_to_quadrants() {
        assert_eq!(default_layout("bogus", 3), presets::quadrants(3));
        assert_eq!(
            default_layout("grid", 2),
            presets::by_name("grid", 2).unwrap()
        );
    }

    #[test]
    fn layout_file_accepts_preset_and_full_documents() {
        let dir = std::env::temp_dir().join(format!("partydeck-layout-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let preset = dir.join("preset.json");
        std::fs::write(&preset, r#"{"preset":"grid"}"#).unwrap();
        assert_eq!(
            layout_from_file(&preset, 4).unwrap(),
            presets::by_name("grid", 4).unwrap()
        );

        let full = dir.join("full.json");
        std::fs::write(&full, serde_json::to_string(&presets::halves_h()).unwrap()).unwrap();
        assert_eq!(layout_from_file(&full, 2).unwrap(), presets::halves_h());

        let bad = dir.join("bad.json");
        std::fs::write(&bad, r#"{"preset":"nope"}"#).unwrap();
        assert!(layout_from_file(&bad, 2).is_err());
        assert!(layout_from_file(&dir.join("missing.json"), 2).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn guests_get_distinct_prefixed_names_and_named_profiles_keep_theirs() {
        let mut instances = vec![
            Instance::new(vec![0]),
            Instance {
                profile: ProfileChoice::Named("Alice".into()),
                ..Instance::new(vec![1])
            },
            Instance::new(vec![2]),
        ];
        assign_profile_names(&mut instances);
        assert_eq!(instances[1].profname, "Alice");
        for guest in [&instances[0], &instances[2]] {
            let name = guest.profname.strip_prefix('.').unwrap();
            assert!(GUEST_NAMES.contains(&name));
        }
        assert_ne!(instances[0].profname, instances[2].profname);
    }

    #[test]
    fn more_guests_than_names_does_not_panic() {
        let mut instances: Vec<Instance> = (0..GUEST_NAMES.len() + 2)
            .map(|i| Instance::new(vec![i]))
            .collect();
        assign_profile_names(&mut instances);
        assert!(instances.iter().all(|i| i.profname.starts_with('.')));
    }

    #[test]
    fn specs_map_slots_to_devices() {
        let profiles = vec!["Alice".to_string(), "Bob".to_string()];
        let devices = vec![pad(Some(1), true), pad(Some(0), true), pad(Some(2), false)];
        let instances = instances_from_specs(
            &[spec("Alice", 0), spec("Bob", 1)],
            &profiles,
            &devices,
            false,
        )
        .unwrap();
        assert_eq!(instances[0].devices, vec![1]);
        assert_eq!(instances[0].pad_slot, Some(0));
        assert_eq!(instances[0].profile, ProfileChoice::Named("Alice".into()));
        assert_eq!(instances[1].devices, vec![0]);

        assert!(instances_from_specs(&[spec("Carol", 0)], &profiles, &devices, false).is_err());
        assert!(
            instances_from_specs(
                &[spec("Alice", 0), spec("Bob", 0)],
                &profiles,
                &devices,
                false
            )
            .is_err()
        );
        assert!(instances_from_specs(&[spec("Alice", 2)], &profiles, &devices, false).is_err());
        let deferred =
            instances_from_specs(&[spec("Alice", 2)], &profiles, &devices, true).unwrap();
        assert!(deferred[0].devices.is_empty());
        assert_eq!(deferred[0].pad_slot, Some(2));
    }

    #[test]
    fn low_resolution_instances_are_upscaled() {
        let mut instances = vec![Instance::new(vec![0]), Instance::new(vec![1])];
        let cfg = PartyConfig::default();
        let layout = presets::halves_h();
        let monitor = Monitor::new("test", 1920, 1080);
        set_instance_resolutions_from_layout(&mut instances, &monitor, &layout, &cfg);
        assert!(instances.iter().all(|i| i.height >= MIN_INSTANCE_HEIGHT));
        assert!(instances.iter().all(|i| i.width > 0));
    }
}
