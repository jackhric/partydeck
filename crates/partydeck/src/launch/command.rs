use std::path::{Path, PathBuf};
use std::process::Command;

use super::goldberg;
use super::log::SessionLog;
use super::mounts;
use super::proton::{self, OsFmt};
use super::runtime::SteamRuntime;
use super::sandbox::{NullPath, ProfileMount, Sandbox, bwrap_args};
use super::sdl;
use crate::compositor::Compositor;
use crate::config::PartyConfig;
use crate::error::Result;
use crate::handler::Handler;
use crate::handler::args::{ArgContext, substitute_args};
use crate::input::proxy::ProxySession;
use crate::input::{DeviceInfo, DeviceType};
use crate::instance::Instance;
use crate::paths::{BIN_GSC_KBM, BIN_UMU_RUN, tmp_dir};
use crate::profile::profile_dir;
use crate::steam::PATH_STEAM;

pub struct CommandContext<'a> {
    pub handler: &'a Handler,
    pub instances: &'a [Instance],
    pub devices: &'a [DeviceInfo],
    pub cfg: &'a PartyConfig,
    pub log: Option<&'a SessionLog>,
    pub comp: &'a Compositor,
    pub proxies: Option<&'a ProxySession>,
}

/// One `gamescope ... -- bwrap ... <runtime> <exe> <args>` command per instance.
pub fn build_commands(ctx: &CommandContext) -> Result<Vec<Command>> {
    let gamescope = gamescope_binary(ctx.cfg)?;
    check_runtime_installed(ctx.handler)?;
    (0..ctx.instances.len())
        .map(|i| build_instance_command(ctx, i, &gamescope))
        .collect()
}

fn gamescope_binary(cfg: &PartyConfig) -> Result<PathBuf> {
    if cfg.kbm_support {
        if !BIN_GSC_KBM.exists() {
            return Err(
                "gamescope-kbm is missing. Please reinstall partydeck or disable KBM support."
                    .into(),
            );
        }
        return Ok(BIN_GSC_KBM.clone());
    }
    if pathsearch::find_executable_in_path("gamescope").is_none() {
        return Err("gamescope not found in PATH. Please install gamescope through your distro's package manager.".into());
    }
    Ok(PathBuf::from("gamescope"))
}

fn check_runtime_installed(h: &Handler) -> Result<()> {
    if let Some(runtime) = SteamRuntime::from_name(&h.runtime)
        && !runtime.exists(&PATH_STEAM)
    {
        return Err(format!(
            "Steam Runtime {} not found! Runtime must be installed on the same drive that the Steam client is installed on.",
            h.runtime
        )
        .into());
    }
    Ok(())
}

fn build_instance_command(ctx: &CommandContext, i: usize, gamescope: &Path) -> Result<Command> {
    let (h, cfg, instance) = (ctx.handler, ctx.cfg, &ctx.instances[i]);
    let win = h.win();

    let gamedir = if mounts::gamedirs_are_mounted(h, cfg) {
        mounts::mounted_gamedir(i)
    } else {
        PathBuf::from(h.get_game_rootpath()?)
    };
    let path_exec = gamedir.join(&h.exec);
    if !path_exec.exists() {
        return Err(format!("Executable not found: {}", path_exec.display()).into());
    }
    let cwd = path_exec.parent().ok_or("couldn't get parent")?;
    let path_prof = profile_dir(&instance.profname);
    let path_pfx = proton::prefix_dir(cfg, i);

    let mut cmd = Command::new(gamescope);
    cmd.current_dir(cwd);

    sdl::apply_env(&mut cmd, h, cfg);
    cmd.env("ENABLE_GAMESCOPE_WSI", "0");
    if win {
        let log_dir = ctx.log.map(|log| log.proton_log_dir(i));
        proton::apply_env(&mut cmd, cfg, h, &path_pfx, log_dir);
    }
    for (key, value) in h.env.split_whitespace().filter_map(|v| v.split_once('=')) {
        cmd.env(key, value);
    }

    gamescope_args(&mut cmd, ctx, i);
    cmd.arg("--");

    let goldberg_mounts = if h.use_goldberg {
        Some(goldberg::mounts(&PATH_STEAM, win, &path_pfx)?)
    } else {
        None
    };
    let sandbox = Sandbox {
        instance,
        devices: ctx.devices,
        proxy_nodes: ctx.proxies.map(|p| {
            p.instance_dev_nodes(i)
                .into_iter()
                .map(str::to_string)
                .collect()
        }),
        mask_hidraw: h.enable_hidraw,
        profile: profile_mount(h, cfg, win, &path_prof, &path_pfx),
        null_paths: null_paths(h, &gamedir),
        null_dir: tmp_dir().join("null"),
        unset_vk_driver_files: std::env::var_os("APPIMAGE").is_some(),
        goldberg: goldberg_mounts,
    };
    cmd.args(bwrap_args(&sandbox));
    if cfg.profile_unique_dirs && !win {
        cmd.env("HOME", path_prof.join("home"));
    }
    if h.use_goldberg {
        goldberg::apply_env(&mut cmd, h, instance, &path_prof);
    }

    if win {
        cmd.arg(&*BIN_UMU_RUN);
    } else if let Some(runtime) = SteamRuntime::from_name(&h.runtime) {
        cmd.args(runtime.command_prefix(&PATH_STEAM));
    }
    cmd.arg(&path_exec);

    let gamedir_fmt = gamedir.os_fmt(win);
    let handlerdir_fmt = h.path_handler.os_fmt(win);
    cmd.args(substitute_args(
        &h.args,
        &ArgContext {
            profile: &instance.profname,
            width: instance.width,
            height: instance.height,
            instance_count: ctx.instances.len(),
            instance_num: i,
            gamedir: &gamedir_fmt,
            handlerdir: &handlerdir_fmt,
        },
    ));
    Ok(cmd)
}

fn gamescope_args(cmd: &mut Command, ctx: &CommandContext, i: usize) {
    let (h, cfg, instance) = (ctx.handler, ctx.cfg, &ctx.instances[i]);
    if h.use_mangohud {
        cmd.arg("--mangoapp");
    }
    cmd.args([
        "-W",
        &instance.width.to_string(),
        "-H",
        &instance.height.to_string(),
    ]);
    if cfg.gamescope_force_grab_cursor {
        cmd.arg("--force-grab-cursor");
    }
    // The instance renders into the compositor's per-player socket, so it
    // must use the Wayland SDL backend and never a physical display.
    cmd.env("WAYLAND_DISPLAY", ctx.comp.player_socket(i));
    cmd.env("SDL_VIDEODRIVER", "wayland");
    cmd.env_remove("DISPLAY");
    cmd.arg("--backend=sdl");

    if !cfg.kbm_support {
        return;
    }
    let kbm_devices: Vec<&DeviceInfo> = instance
        .devices
        .iter()
        .filter_map(|&d| ctx.devices.get(d))
        .filter(|dev| matches!(dev.device_type, DeviceType::Keyboard | DeviceType::Mouse))
        .collect();
    if kbm_devices
        .iter()
        .any(|dev| dev.device_type == DeviceType::Keyboard)
    {
        cmd.arg("--backend-disable-keyboard");
    }
    if kbm_devices
        .iter()
        .any(|dev| dev.device_type == DeviceType::Mouse)
    {
        cmd.arg("--backend-disable-mouse");
    }
    if !kbm_devices.is_empty() {
        let held: String = kbm_devices
            .iter()
            .map(|dev| format!("{},", dev.path))
            .collect();
        cmd.arg(format!("--libinput-hold-dev={held}"));
        cmd.arg("--grab");
    }
}

fn profile_mount(
    h: &Handler,
    cfg: &PartyConfig,
    win: bool,
    path_prof: &Path,
    path_pfx: &Path,
) -> Option<ProfileMount> {
    if !cfg.profile_unique_dirs {
        return None;
    }
    if win {
        return Some(ProfileMount::Windows {
            windata: path_prof.join("windata"),
            prefix_user: path_pfx.join("drive_c/users/steamuser"),
        });
    }
    // Steam runtimes look for HOME/.steam, so the real Steam dir is bound into
    // the per-profile HOME.
    let needs_steam = !h.runtime.is_empty() || h.steam_appid.is_some();
    Some(ProfileMount::Linux {
        steam: needs_steam.then(|| (PATH_STEAM.clone(), path_prof.join("home/.steam"))),
    })
}

fn null_paths(h: &Handler, gamedir: &Path) -> Vec<NullPath> {
    h.game_null_paths
        .iter()
        .map(|subpath| gamedir.join(subpath))
        .filter_map(|path| {
            if path.is_file() {
                Some(NullPath::File(path))
            } else if path.is_dir() {
                Some(NullPath::Dir(path))
            } else {
                None
            }
        })
        .collect()
}
