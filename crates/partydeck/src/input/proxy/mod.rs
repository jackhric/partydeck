//! Per-player proxy gamepads. Steam Input destroys and recreates its virtual
//! pad on controller churn, so each game instead gets a stable uinput pad we
//! own; a router thread per player forwards events and force feedback, and
//! re-attaches to the slot-N Steam pad whenever it reappears.

mod pad;
mod router;
#[cfg(test)]
mod tests;

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use pad::{build_proxy_pad, wait_for_dev_nodes};
use router::Router;

use crate::config::PartyConfig;
use crate::handler::Handler;
use crate::input::{DeviceInfo, DeviceType};
use crate::instance::Instance;

pub struct ProxySession {
    players: Vec<PlayerProxy>,
}

struct PlayerProxy {
    instance: usize,
    dev_nodes: Vec<String>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

// (slot, source path) pairs to route for an instance. An explicit pad_slot
// yields exactly one router even when no source device exists yet; the router
// attaches once the slot-N pad appears.
fn instance_pads<'a>(
    instance: &Instance,
    devices: &'a [DeviceInfo],
) -> Vec<(u32, Option<&'a str>)> {
    let mut gamepads = instance
        .devices
        .iter()
        .filter_map(|&d| devices.get(d))
        .filter(|dev| dev.enabled && dev.device_type == DeviceType::Gamepad);
    match instance.pad_slot {
        Some(slot) => vec![(slot, gamepads.next().map(|dev| dev.path.as_str()))],
        None => gamepads
            .filter_map(|dev| dev.xinput_slot.map(|slot| (slot, Some(dev.path.as_str()))))
            .collect(),
    }
}

impl ProxySession {
    /// Proxies are only usable when every gamepad in play is a Steam Input pad
    /// (or the instance names its slot explicitly).
    pub fn wanted(
        cfg: &PartyConfig,
        handler: &Handler,
        instances: &[Instance],
        devices: &[DeviceInfo],
    ) -> bool {
        cfg.proxy_gamepads
            && !handler.enable_hidraw
            && instances.iter().all(|instance| {
                instance.pad_slot.is_some()
                    || instance.devices.iter().all(|&d| {
                        devices.get(d).is_none_or(|dev| {
                            !dev.enabled
                                || dev.device_type != DeviceType::Gamepad
                                || dev.xinput_slot.is_some()
                        })
                    })
            })
    }

    pub fn start(instances: &[Instance], devices: &[DeviceInfo]) -> io::Result<ProxySession> {
        let mut session = ProxySession {
            players: Vec::new(),
        };
        for (i, instance) in instances.iter().enumerate() {
            for (slot, source_path) in instance_pads(instance, devices) {
                let mut proxy = build_proxy_pad(slot)?;
                let dev_nodes = wait_for_dev_nodes(&mut proxy)?;
                let shutdown = Arc::new(AtomicBool::new(false));
                let router = Router::new(proxy, slot, source_path, shutdown.clone());
                let thread = std::thread::spawn(move || router.run());
                session.players.push(PlayerProxy {
                    instance: i,
                    dev_nodes,
                    shutdown,
                    thread: Some(thread),
                });
            }
        }
        Ok(session)
    }

    pub fn instance_dev_nodes(&self, instance: usize) -> Vec<&str> {
        self.players
            .iter()
            .filter(|p| p.instance == instance)
            .flat_map(|p| p.dev_nodes.iter().map(String::as_str))
            .collect()
    }
}

impl Drop for ProxySession {
    fn drop(&mut self) {
        for player in &self.players {
            player.shutdown.store(true, Ordering::Relaxed);
        }
        // The kernel destroys the uinput nodes when each router drops its
        // VirtualDevice, so joining is the whole cleanup.
        for player in &mut self.players {
            if let Some(thread) = player.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
