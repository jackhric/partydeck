//! Minimal overlay client: transparent fullscreen HUD test pattern with an
//! animated corner badge. Connect it to the compositor's -overlay socket:
//!   WAYLAND_DISPLAY=<prefix>-overlay cargo run -p partydeck-comp --example overlay_test

use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;

use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_region, wl_registry, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

const W: usize = 1280;
const H: usize = 800;
const STRIDE: usize = W * 4;

struct App {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    surface: Option<wl_surface::WlSurface>,
    configured: bool,
    running: bool,
    frame: u64,
}

fn premul(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    let m = |c: u8| ((c as u32 * a as u32) / 255) as u8;
    [m(b), m(g), m(r), a] // ARGB8888 little-endian = B,G,R,A
}

fn draw(file: &std::fs::File, offset: u64, frame: u64) {
    let mut buf = vec![0u8; STRIDE * H]; // fully transparent

    // top HUD bar
    let bar = premul(12, 18, 28, 178);
    for y in 0..56 {
        for x in 0..W {
            buf[y * STRIDE + x * 4..y * STRIDE + x * 4 + 4].copy_from_slice(&bar);
        }
    }
    // hollow center rectangle for alignment
    let edge = premul(77, 140, 242, 255);
    for x in 340..940 {
        for y in [300usize, 301, 498, 499] {
            buf[y * STRIDE + x * 4..y * STRIDE + x * 4 + 4].copy_from_slice(&edge);
        }
    }
    for y in 300..500 {
        for x in [340usize, 341, 938, 939] {
            buf[y * STRIDE + x * 4..y * STRIDE + x * 4 + 4].copy_from_slice(&edge);
        }
    }
    // pulsing corner badge
    let pulse = ((frame as f32 * 0.12).sin() * 0.5 + 0.5 * 1.0) * 0.6 + 0.4;
    let badge = premul(77, 140, 242, (255.0 * pulse) as u8);
    for y in H - 90..H - 30 {
        for x in W - 230..W - 30 {
            buf[y * STRIDE + x * 4..y * STRIDE + x * 4 + 4].copy_from_slice(&badge);
        }
    }

    file.write_at(&buf, offset).unwrap();
}

impl Dispatch<wl_registry::WlRegistry, ()> for App {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor =
                        Some(registry.bind::<wl_compositor::WlCompositor, _, _>(name, version.min(4), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ()));
                }
                "xdg_wm_base" => {
                    state.wm_base =
                        Some(registry.bind::<xdg_wm_base::XdgWmBase, _, _>(name, version.min(2), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for App {
    fn event(
        _: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for App {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for App {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel::Event::Close = event {
            state.running = false;
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for App {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.frame += 1;
        }
    }
}

macro_rules! ignore {
    ($($t:ty),*) => {$(
        impl Dispatch<$t, ()> for App {
            fn event(_: &mut Self, _: &$t, _: <$t as wayland_client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
ignore!(
    wl_compositor::WlCompositor,
    wl_shm::WlShm,
    wl_shm_pool::WlShmPool,
    wl_buffer::WlBuffer,
    wl_surface::WlSurface,
    wl_region::WlRegion
);

fn main() {
    let conn = Connection::connect_to_env().expect("connect to overlay socket");
    let mut queue = conn.new_event_queue::<App>();
    let qh = queue.handle();
    let display = conn.display();
    display.get_registry(&qh, ());

    let mut app = App {
        compositor: None,
        shm: None,
        wm_base: None,
        surface: None,
        configured: false,
        running: true,
        frame: 0,
    };
    queue.roundtrip(&mut app).unwrap();

    let compositor = app.compositor.clone().expect("wl_compositor");
    let shm = app.shm.clone().expect("wl_shm");
    let wm_base = app.wm_base.clone().expect("xdg_wm_base");

    let surface = compositor.create_surface(&qh, ());
    // click-through: pointer input never lands on the overlay
    let empty = compositor.create_region(&qh, ());
    surface.set_input_region(Some(&empty));

    let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
    let toplevel = xdg_surface.get_toplevel(&qh, ());
    toplevel.set_title("partydeck-overlay-test".into());
    toplevel.set_app_id("partydeck-overlay".into());
    surface.commit();
    app.surface = Some(surface.clone());

    while !app.configured {
        queue.blocking_dispatch(&mut app).unwrap();
    }

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap();
    let path = format!("{runtime_dir}/overlay-test-shm-{}", std::process::id());
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    let pool_size = STRIDE * H * 2;
    file.set_len(pool_size as u64).unwrap();

    let pool = shm.create_pool(file.as_fd(), pool_size as i32, &qh, ());
    let buffers = [
        pool.create_buffer(0, W as i32, H as i32, STRIDE as i32, wl_shm::Format::Argb8888, &qh, ()),
        pool.create_buffer((STRIDE * H) as i32, W as i32, H as i32, STRIDE as i32, wl_shm::Format::Argb8888, &qh, ()),
    ];

    println!("overlay_test: connected, drawing");
    let mut last_frame = 0;
    while app.running {
        let idx = (app.frame % 2) as usize;
        draw(&file, (idx * STRIDE * H) as u64, app.frame);
        surface.attach(Some(&buffers[idx]), 0, 0);
        surface.damage_buffer(0, 0, W as i32, H as i32);
        surface.frame(&qh, ());
        surface.commit();
        let target = app.frame + 1;
        while app.running && app.frame < target {
            queue.blocking_dispatch(&mut app).unwrap();
        }
        if app.frame - last_frame >= 120 {
            println!("overlay_test: {} frames", app.frame);
            last_frame = app.frame;
        }
    }
}
