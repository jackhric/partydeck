use partydeck_comp_proto::layout::PixelRect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub x: i32,
    pub y: i32,
    pub scale: f64,
}

// Slot windows are contain-fitted: nested gamescope never honors resize
// configures, so whatever buffer size an instance was launched with gets
// scaled (up or down) and centred in its slot, then cropped to it.
pub fn contain(slot: PixelRect, w: i32, h: i32) -> Option<Fit> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let scale = (slot.w as f64 / w as f64).min(slot.h as f64 / h as f64);
    Some(Fit {
        x: slot.x + ((slot.w as f64 - w as f64 * scale) / 2.0) as i32,
        y: slot.y + ((slot.h as f64 - h as f64 * scale) / 2.0) as i32,
        scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOT: PixelRect = PixelRect {
        x: 640,
        y: 400,
        w: 640,
        h: 400,
    };

    #[test]
    fn exact_size_is_identity() {
        assert_eq!(
            contain(SLOT, 640, 400),
            Some(Fit {
                x: 640,
                y: 400,
                scale: 1.0
            })
        );
    }

    #[test]
    fn oversized_same_aspect_scales_down_to_origin() {
        assert_eq!(
            contain(SLOT, 1280, 800),
            Some(Fit {
                x: 640,
                y: 400,
                scale: 0.5
            })
        );
    }

    #[test]
    fn narrow_buffer_is_pillarboxed() {
        let fit = contain(SLOT, 800, 800).unwrap();
        assert_eq!(fit.scale, 0.5);
        assert_eq!((fit.x, fit.y), (640 + 120, 400));
    }

    #[test]
    fn wide_buffer_is_letterboxed() {
        let fit = contain(SLOT, 1280, 400).unwrap();
        assert_eq!(fit.scale, 0.5);
        assert_eq!((fit.x, fit.y), (640, 400 + 100));
    }

    #[test]
    fn small_buffer_scales_up() {
        assert_eq!(
            contain(SLOT, 320, 200),
            Some(Fit {
                x: 640,
                y: 400,
                scale: 2.0
            })
        );
    }

    #[test]
    fn degenerate_buffer_has_no_fit() {
        assert_eq!(contain(SLOT, 0, 400), None);
        assert_eq!(contain(SLOT, 640, -1), None);
    }
}
