use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::utils::CommitCounter;
use smithay::desktop::Window;
use smithay::backend::renderer::Color32F;
use smithay::render_elements;
use smithay::utils::{Physical, Point, Rectangle, Size, Transform};

use partydeck_comp::layout::Layout;

render_elements! {
    pub OverlayElement<=GlesRenderer>;
    Solid=SolidColorRenderElement,
    Texture=MemoryRenderBufferRenderElement<GlesRenderer>,
}

const WAITING_FILL: [f32; 4] = [0.07, 0.08, 0.11, 1.0];
const FOCUS_COLOR: [f32; 4] = [0.30, 0.55, 0.95, 1.0];
const FOCUS_THICKNESS: i32 = 3;
const CARD_FALLBACK: [f32; 4] = [0.16, 0.20, 0.28, 1.0];
const CARD_FALLBACK_SIZE: (i32, i32) = (160, 160);

pub struct Assets {
    pub loading: Option<(MemoryRenderBuffer, (i32, i32))>,
}

impl Assets {
    pub fn load() -> Self {
        Self { loading: load_png("loading.png") }
    }
}

fn load_png(name: &str) -> Option<(MemoryRenderBuffer, (i32, i32))> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("res/comp").join(name));
            candidates.push(dir.join("../res/comp").join(name));
        }
    }
    candidates.push(std::path::PathBuf::from("res/comp").join(name));

    let path = candidates.iter().find(|p| p.exists())?;
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = (img.width() as i32, img.height() as i32);
    let buffer = MemoryRenderBuffer::from_slice(
        &img,
        smithay::backend::allocator::Fourcc::Abgr8888,
        (w, h),
        1,
        Transform::Normal,
        None,
    );
    eprintln!("[comp] overlay asset loaded: {}", path.display());
    Some((buffer, (w, h)))
}

pub fn build(
    layout: &Layout,
    slot_windows: &[Option<Window>],
    assets: &Assets,
    start_time: std::time::Instant,
    size: Size<i32, Physical>,
    renderer: &mut GlesRenderer,
) -> Vec<OverlayElement> {
    let rects = layout.resolve(size.w.max(1) as u32, size.h.max(1) as u32);
    let mut elements = Vec::new();

    let t = start_time.elapsed().as_secs_f32();
    let pulse = 0.65 + 0.35 * (t * 3.0).sin().abs();

    for (i, r) in rects.iter().enumerate() {
        let live = slot_windows.get(i).map(|w| w.is_some()).unwrap_or(false);
        let rect = Rectangle::new((r.x, r.y).into(), (r.w, r.h).into());

        if i == layout.focus && rects.len() > 1 {
            for strip in border_strips(rect, FOCUS_THICKNESS) {
                elements.push(OverlayElement::Solid(SolidColorRenderElement::new(
                    Id::new(),
                    strip,
                    CommitCounter::default(),
                    Color32F::from(FOCUS_COLOR),
                    Kind::Unspecified,
                )));
            }
        }

        if !live {
            let card = assets.loading.as_ref().and_then(|(buffer, (w, h))| {
                let loc: Point<f64, smithay::utils::Physical> =
                    ((r.x + (r.w - w) / 2) as f64, (r.y + (r.h - h) / 2) as f64).into();
                MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    loc,
                    buffer,
                    Some(pulse),
                    None,
                    None,
                    Kind::Unspecified,
                )
                .ok()
            });
            match card {
                Some(el) => elements.push(OverlayElement::Texture(el)),
                None => {
                    let (cw, ch) = CARD_FALLBACK_SIZE;
                    let card_rect = Rectangle::new(
                        (r.x + (r.w - cw) / 2, r.y + (r.h - ch) / 2).into(),
                        (cw, ch).into(),
                    );
                    let mut color = CARD_FALLBACK;
                    color[0] *= pulse;
                    color[1] *= pulse;
                    color[2] *= pulse;
                    elements.push(OverlayElement::Solid(SolidColorRenderElement::new(
                        Id::new(),
                        card_rect,
                        CommitCounter::default(),
                        Color32F::from(color),
                        Kind::Unspecified,
                    )));
                }
            }
            elements.push(OverlayElement::Solid(SolidColorRenderElement::new(
                Id::new(),
                rect,
                CommitCounter::default(),
                Color32F::from(WAITING_FILL),
                Kind::Unspecified,
            )));
        }
    }

    elements
}

fn border_strips(
    rect: Rectangle<i32, smithay::utils::Physical>,
    t: i32,
) -> [Rectangle<i32, smithay::utils::Physical>; 4] {
    let (x, y, w, h) = (rect.loc.x, rect.loc.y, rect.size.w, rect.size.h);
    [
        Rectangle::new((x, y).into(), (w, t).into()),
        Rectangle::new((x, y + h - t).into(), (w, t).into()),
        Rectangle::new((x, y + t).into(), (t, h - 2 * t).into()),
        Rectangle::new((x + w - t, y + t).into(), (t, h - 2 * t).into()),
    ]
}
