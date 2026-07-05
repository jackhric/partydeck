mod backend;
mod control;
mod focus;
mod handlers;
mod render;
mod slots;
mod state;

use std::io::Write;
use std::time::Duration;

use clap::Parser;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::{Display, DisplayHandle};

use state::CompState;

#[derive(Parser)]
#[command(name = "partydeck-comp")]
struct Args {
    /// Per-player sockets are created as <prefix>-p0 .. <prefix>-pN-1.
    #[arg(long, default_value = "partydeck-comp")]
    socket_prefix: String,
    /// Layout JSON file; slot count defines the number of player sockets.
    #[arg(long)]
    layout: Option<std::path::PathBuf>,
    /// Player count for the default quadrants layout when --layout is absent.
    #[arg(long, default_value_t = 1)]
    players: usize,
    #[arg(long)]
    fullscreen: bool,
    #[arg(long, default_value = "1280x800", value_parser = parse_size)]
    size: (u32, u32),
    /// Overlay split-line style: "off" | "faint" | "medium" | "strong".
    #[arg(long, default_value = "faint")]
    border: String,
}

fn load_layout(args: &Args) -> Result<partydeck_comp_proto::layout::Layout, String> {
    let layout = match &args.layout {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read layout {}: {e}", path.display()))?;
            serde_json::from_str(&text).map_err(|e| format!("invalid layout json: {e}"))?
        }
        None => partydeck_comp_proto::presets::quadrants(args.players),
    };
    let layout: partydeck_comp_proto::layout::Layout = layout;
    layout.validate(layout.slots.len())?;
    Ok(layout)
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x').ok_or("expected WxH")?;
    Ok((
        w.parse().map_err(|_| "bad width")?,
        h.parse().map_err(|_| "bad height")?,
    ))
}

pub struct CalloopData {
    state: CompState,
    display_handle: DisplayHandle,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;
    let display: Display<CompState> = Display::new()?;
    let display_handle = display.handle();

    let layout = load_layout(&args)?;
    let (backend, winit) = backend::winit::init(&display_handle, args.size, args.fullscreen)?;
    let state = CompState::new(&mut event_loop, display, &args.socket_prefix, layout, args.border, backend);
    let mut data = CalloopData { state, display_handle };

    backend::winit::insert_source(&mut event_loop, winit)?;

    // Render on our own clock, never on host frame callbacks: hosts (gamescope
    // especially) pace callbacks to their own compositing schedule, and a
    // callback-driven loop quantizes every client below us to a fraction of
    // the refresh rate. Nested kwin_wayland works the same way.
    let frame_interval = Duration::from_nanos(16_666_667);
    let mut next_frame = std::time::Instant::now();
    event_loop
        .handle()
        .insert_source(Timer::immediate(), move |_, _, data| {
            render::tick_children(&mut data.state, &mut data.display_handle);
            if data.state.host_ready {
                data.state.host_ready = false;
                render::redraw(&mut data.state, &mut data.display_handle);
            }
            let now = std::time::Instant::now();
            next_frame += frame_interval;
            if next_frame < now {
                next_frame = now + frame_interval;
            }
            TimeoutAction::ToInstant(next_frame)
        })
        .map_err(|e| format!("failed to insert render timer: {e}"))?;

    let control_path = control::init(&mut event_loop, &args.socket_prefix)?;

    println!("READY");
    for name in &data.state.socket_names {
        println!("socket={}", name.to_string_lossy());
    }
    println!("control={}", control_path.display());
    std::io::stdout().flush()?;

    // Redraws can be throttled by the host while hidden; flush every turn so
    // protocol replies never starve.
    event_loop.run(None, &mut data, |data| {
        let _ = data.display_handle.flush_clients();
    })?;

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let dir = std::path::Path::new(&runtime_dir);
        for name in &data.state.socket_names {
            let _ = std::fs::remove_file(dir.join(name));
            let mut lock = name.clone();
            lock.push(".lock");
            let _ = std::fs::remove_file(dir.join(lock));
        }
        let _ = std::fs::remove_file(&control_path);
        let mut ctl_lock = control_path.clone().into_os_string();
        ctl_lock.push(".lock");
        let _ = std::fs::remove_file(ctl_lock);
    }
    Ok(())
}
