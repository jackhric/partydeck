use std::path::{Path, PathBuf};

use super::goldberg::GoldbergMounts;
use crate::input::{DeviceInfo, DeviceType};
use crate::instance::Instance;

pub enum NullPath {
    File(PathBuf),
    Dir(PathBuf),
}

pub enum ProfileMount {
    Windows {
        windata: PathBuf,
        prefix_user: PathBuf,
    },
    Linux {
        steam: Option<(PathBuf, PathBuf)>,
    },
}

pub struct Sandbox<'a> {
    pub instance: &'a Instance,
    pub devices: &'a [DeviceInfo],
    /// Some when proxy pads are in use: only these nodes are visible in /dev/input.
    pub proxy_nodes: Option<Vec<String>>,
    pub mask_hidraw: bool,
    pub profile: Option<ProfileMount>,
    pub null_paths: Vec<NullPath>,
    pub null_dir: PathBuf,
    pub unset_vk_driver_files: bool,
    pub goldberg: Option<GoldbergMounts>,
}

/// The complete bwrap argument list, starting with the `bwrap` program name.
/// Order matters: later binds shadow earlier ones.
pub fn bwrap_args(sandbox: &Sandbox) -> Vec<String> {
    let mut args = Args::default();
    args.push(["bwrap", "--die-with-parent"]);
    args.push(["--dev-bind", "/", "/"]);
    args.push(["--tmpfs", "/tmp"]);

    match &sandbox.proxy_nodes {
        Some(nodes) => {
            args.push(["--tmpfs", "/dev/input"]);
            for node in nodes {
                args.push(["--dev-bind", node, node]);
            }
        }
        None => mask_foreign_devices(&mut args, sandbox),
    }

    match &sandbox.profile {
        Some(ProfileMount::Windows {
            windata,
            prefix_user,
        }) => {
            args.bind(windata, prefix_user);
        }
        Some(ProfileMount::Linux {
            steam: Some((from, to)),
        }) => args.bind(from, to),
        Some(ProfileMount::Linux { steam: None }) | None => {}
    }

    for null in &sandbox.null_paths {
        match null {
            NullPath::File(path) => args.bind(Path::new("/dev/null"), path),
            NullPath::Dir(path) => args.bind(&sandbox.null_dir, path),
        }
    }

    if sandbox.unset_vk_driver_files {
        // /tmp is faked above, so the AppImage's VK_DRIVER_FILES would point at
        // nothing; unsetting it makes the game use the real Vulkan ICD dir.
        args.push(["--unsetenv", "VK_DRIVER_FILES"]);
    }

    if let Some(goldberg) = &sandbox.goldberg {
        // Steam exports a 64-bit SteamOverlayGameId for our shortcut; Goldberg
        // parses it as an out-of-range int and aborts. Drop it.
        args.push(["--unsetenv", "SteamOverlayGameId"]);
        for (from, to) in &goldberg.binds {
            args.bind(from, to);
        }
    }

    args.0
}

// Every enabled device is visible unless it is a gamepad belonging to another
// instance. Wine's winebus reads controllers via /dev/hidraw* when hidraw is
// exposed, so masking only the evdev node would leak input to every instance.
fn mask_foreign_devices(args: &mut Args, sandbox: &Sandbox) {
    for (d, dev) in sandbox.devices.iter().enumerate() {
        let foreign_gamepad =
            !sandbox.instance.devices.contains(&d) && dev.device_type == DeviceType::Gamepad;
        if !dev.enabled || foreign_gamepad {
            args.push(["--bind", "/dev/null", &dev.path]);
            if sandbox.mask_hidraw {
                for hidraw in &dev.hidraw_paths {
                    args.push(["--bind", "/dev/null", hidraw]);
                }
            }
        }
    }
}

#[derive(Default)]
struct Args(Vec<String>);

impl Args {
    fn push<const N: usize>(&mut self, items: [&str; N]) {
        self.0.extend(items.iter().map(|s| s.to_string()));
    }

    fn bind(&mut self, from: &Path, to: &Path) {
        self.push(["--bind", &from.to_string_lossy(), &to.to_string_lossy()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(path: &str, device_type: DeviceType, enabled: bool, hidraw: &[&str]) -> DeviceInfo {
        DeviceInfo {
            path: path.to_string(),
            hidraw_paths: hidraw.iter().map(|s| s.to_string()).collect(),
            enabled,
            device_type,
            xinput_slot: None,
        }
    }

    fn sandbox<'a>(instance: &'a Instance, devices: &'a [DeviceInfo]) -> Sandbox<'a> {
        Sandbox {
            instance,
            devices,
            proxy_nodes: None,
            mask_hidraw: false,
            profile: None,
            null_paths: Vec::new(),
            null_dir: PathBuf::from("/tmp/null"),
            unset_vk_driver_files: false,
            goldberg: None,
        }
    }

    const BASE: [&str; 8] = [
        "bwrap",
        "--die-with-parent",
        "--dev-bind",
        "/",
        "/",
        "--tmpfs",
        "/tmp",
        "--bind",
    ];

    #[test]
    fn masks_other_players_gamepads_and_disabled_devices_only() {
        let devices = vec![
            dev(
                "/dev/input/event0",
                DeviceType::Gamepad,
                true,
                &["/dev/hidraw0"],
            ),
            dev(
                "/dev/input/event1",
                DeviceType::Gamepad,
                true,
                &["/dev/hidraw1"],
            ),
            dev("/dev/input/event2", DeviceType::Keyboard, true, &[]),
            dev("/dev/input/event3", DeviceType::Mouse, false, &[]),
        ];
        let instance = Instance::new(vec![0, 2]);
        let args = bwrap_args(&sandbox(&instance, &devices));
        assert_eq!(&args[..7], &BASE[..7]);
        assert_eq!(
            &args[7..],
            &[
                "--bind",
                "/dev/null",
                "/dev/input/event1",
                "--bind",
                "/dev/null",
                "/dev/input/event3",
            ]
        );
    }

    #[test]
    fn hidraw_siblings_are_masked_with_the_evdev_node() {
        let devices = vec![
            dev("/dev/input/event0", DeviceType::Gamepad, true, &[]),
            dev(
                "/dev/input/event1",
                DeviceType::Gamepad,
                true,
                &["/dev/hidraw1", "/dev/hidraw2"],
            ),
        ];
        let instance = Instance::new(vec![0]);
        let mut sb = sandbox(&instance, &devices);
        sb.mask_hidraw = true;
        let args = bwrap_args(&sb);
        assert_eq!(
            &args[7..],
            &[
                "--bind",
                "/dev/null",
                "/dev/input/event1",
                "--bind",
                "/dev/null",
                "/dev/hidraw1",
                "--bind",
                "/dev/null",
                "/dev/hidraw2",
            ]
        );
    }

    #[test]
    fn proxy_mode_hides_dev_input_and_binds_only_proxy_nodes() {
        let devices = vec![dev("/dev/input/event0", DeviceType::Gamepad, true, &[])];
        let instance = Instance::new(vec![0]);
        let mut sb = sandbox(&instance, &devices);
        sb.proxy_nodes = Some(vec!["/dev/input/event20".into(), "/dev/input/js1".into()]);
        let args = bwrap_args(&sb);
        assert_eq!(
            &args[7..],
            &[
                "--tmpfs",
                "/dev/input",
                "--dev-bind",
                "/dev/input/event20",
                "/dev/input/event20",
                "--dev-bind",
                "/dev/input/js1",
                "/dev/input/js1",
            ]
        );
        assert!(!args.contains(&"/dev/null".to_string()));
    }

    #[test]
    fn profile_null_paths_and_goldberg_follow_device_masks_in_order() {
        let devices = vec![];
        let instance = Instance::new(vec![]);
        let mut sb = sandbox(&instance, &devices);
        sb.profile = Some(ProfileMount::Windows {
            windata: "/p/windata".into(),
            prefix_user: "/pfx/drive_c/users/steamuser".into(),
        });
        sb.null_paths = vec![
            NullPath::File("/game/a.dll".into()),
            NullPath::Dir("/game/mods".into()),
        ];
        sb.unset_vk_driver_files = true;
        sb.goldberg = Some(GoldbergMounts {
            binds: vec![("/res/goldberg/linux64".into(), "/steam/sdk64".into())],
        });
        let args = bwrap_args(&sb);
        assert_eq!(
            &args[7..],
            &[
                "--bind",
                "/p/windata",
                "/pfx/drive_c/users/steamuser",
                "--bind",
                "/dev/null",
                "/game/a.dll",
                "--bind",
                "/tmp/null",
                "/game/mods",
                "--unsetenv",
                "VK_DRIVER_FILES",
                "--unsetenv",
                "SteamOverlayGameId",
                "--bind",
                "/res/goldberg/linux64",
                "/steam/sdk64",
            ]
        );
    }

    #[test]
    fn linux_profile_without_steam_adds_nothing() {
        let devices = vec![];
        let instance = Instance::new(vec![]);
        let mut sb = sandbox(&instance, &devices);
        sb.profile = Some(ProfileMount::Linux { steam: None });
        assert_eq!(bwrap_args(&sb).len(), 7);
    }
}
