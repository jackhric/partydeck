mod command;
mod goldberg;
mod log;
mod mounts;
mod proton;
pub mod request;
mod runtime;
mod sandbox;
mod sdl;
mod spawn;

pub use mounts::clear_tmp;
pub use proton::erase_prefixes;
pub use request::LaunchRequest;

use partydeck_comp_proto::ipc;

use crate::compositor::Compositor;
use crate::config::PartyConfig;
use crate::error::Result;
use crate::handler::Handler;
use crate::input::DeviceInfo;
use crate::input::proxy::ProxySession;
use crate::instance::Instance;
use crate::profile::{
    create_guest_profile, create_profile_gamesave, read_avatar_base64, remove_guest_profiles,
};
use crate::steam;
use log::SessionLog;

/// Launch sequence shared by the GUI and the `launch` CLI subcommand. Runs
/// synchronously and returns errors for the caller to display or log.
pub fn run_launch(request: LaunchRequest) -> Result<()> {
    let LaunchRequest {
        handler,
        mut instances,
        devices,
        cfg,
        monitor,
        layout,
    } = request;

    layout.validate(instances.len())?;
    request::set_instance_resolutions_from_layout(&mut instances, &monitor, &layout, &cfg);
    let comp = Compositor::spawn(&layout, &monitor, &cfg.border_style)?;

    setup_profiles(&handler, &instances)?;
    let proxies = start_proxies(&cfg, &handler, &instances, &devices);
    announce_slots(&comp, &handler, &instances);
    let log = SessionLog::create(&cfg);

    if mounts::gamedirs_are_mounted(&handler, &cfg) {
        mounts::fuse_overlayfs_mount_gamedirs(&handler, &instances)?;
    }

    let launch_result = spawn::launch_game(&command::CommandContext {
        handler: &handler,
        instances: &instances,
        devices: &devices,
        cfg: &cfg,
        log: log.as_ref(),
        comp: &comp,
        proxies: proxies.as_ref(),
    });

    // Proxies go before the compositor so games never see a dead display with
    // live pads; cleanup runs last regardless of the launch outcome.
    drop(proxies);
    drop(comp);
    cleanup_after_launch();

    launch_result
}

fn setup_profiles(h: &Handler, instances: &[Instance]) -> Result<()> {
    eprintln!("[partydeck] Instances:");
    for instance in instances {
        if instance.profile.is_guest() {
            create_guest_profile(&instance.profname)?;
        }
        if h.is_saved_handler() {
            create_profile_gamesave(&instance.profname, h)?;
        }
        eprintln!(
            "[partydeck] - Profile: {}, Resolution: {}x{}",
            instance.profname, instance.width, instance.height
        );
    }
    Ok(())
}

fn start_proxies(
    cfg: &PartyConfig,
    handler: &Handler,
    instances: &[Instance],
    devices: &[DeviceInfo],
) -> Option<ProxySession> {
    if !ProxySession::wanted(cfg, handler, instances, devices) {
        return None;
    }
    match ProxySession::start(instances, devices) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("[partydeck] proxy pads unavailable, using direct devices: {e}");
            None
        }
    }
}

fn announce_slots(comp: &Compositor, handler: &Handler, instances: &[Instance]) {
    let logo = handler.steam_appid.and_then(steam::app_logo_base64);
    for (i, instance) in instances.iter().enumerate() {
        let display_name = instance
            .profname
            .strip_prefix('.')
            .unwrap_or(&instance.profname);
        let cmd = ipc::Command::SetSlotStatus {
            slot: i,
            status: partydeck_comp_proto::state::SlotStatus::Loading,
            label: Some(display_name.to_string()),
            avatar: read_avatar_base64(&instance.profname),
            logo: logo.clone(),
        };
        if let Err(e) = comp.send(&cmd) {
            eprintln!("[partydeck] failed to set slot {i} status: {e}");
        }
    }
}

fn cleanup_after_launch() {
    if let Err(err) = remove_guest_profiles() {
        eprintln!("[partydeck] Error removing guest profiles: {err}");
    }
    if let Err(err) = clear_tmp() {
        eprintln!("[partydeck] Error removing tmp directory: {err}");
    }
}
