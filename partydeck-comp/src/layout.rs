use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default)]
    pub focus: usize,
    pub slots: Vec<Slot>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

const EDGE_TOLERANCE: f32 = 0.001;

impl Layout {
    pub fn validate(&self, players: usize) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("unsupported layout version {}", self.version));
        }
        if self.slots.len() != players {
            return Err(format!(
                "layout has {} slots but session has {} players",
                self.slots.len(),
                players
            ));
        }
        for (i, slot) in self.slots.iter().enumerate() {
            let r = &slot.rect;
            if r.w <= 0.0 || r.h <= 0.0 {
                return Err(format!("slot {i}: non-positive size"));
            }
            if r.x < 0.0 || r.y < 0.0 {
                return Err(format!("slot {i}: negative origin"));
            }
            if r.x + r.w > 1.0 + EDGE_TOLERANCE || r.y + r.h > 1.0 + EDGE_TOLERANCE {
                return Err(format!("slot {i}: extends past the output"));
            }
        }
        if self.focus >= self.slots.len() {
            return Err(format!("focus {} out of range", self.focus));
        }
        Ok(())
    }

    // Rounds edges, not sizes, so adjacent slots share seams exactly.
    pub fn resolve(&self, width: u32, height: u32) -> Vec<PixelRect> {
        self.slots
            .iter()
            .map(|slot| {
                let r = &slot.rect;
                let x1 = (r.x * width as f32).round() as i32;
                let y1 = (r.y * height as f32).round() as i32;
                let x2 = ((r.x + r.w) * width as f32).round() as i32;
                let y2 = ((r.y + r.h) * height as f32).round() as i32;
                PixelRect {
                    x: x1,
                    y: y1,
                    w: x2 - x1,
                    h: y2 - y1,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    fn covers_exactly(rects: &[PixelRect], w: i32, h: i32) {
        let area: i32 = rects.iter().map(|r| r.w * r.h).sum();
        assert_eq!(area, w * h, "total area must equal output area");
        for (i, a) in rects.iter().enumerate() {
            assert!(a.x >= 0 && a.y >= 0 && a.x + a.w <= w && a.y + a.h <= h, "rect {i} in bounds");
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                let overlap =
                    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!overlap, "rects {i} and {j} overlap");
            }
        }
    }

    #[test]
    fn quadrants_1280x800_tile_exactly() {
        let rects = presets::quadrants(4).resolve(1280, 800);
        covers_exactly(&rects, 1280, 800);
        assert_eq!(rects[0], PixelRect { x: 0, y: 0, w: 640, h: 400 });
        assert_eq!(rects[3], PixelRect { x: 640, y: 400, w: 640, h: 400 });
    }

    #[test]
    fn odd_size_leaves_no_seam_gaps() {
        let rects = presets::quadrants(4).resolve(853, 480);
        covers_exactly(&rects, 853, 480);
        assert_eq!(rects[0].w + rects[1].w, 853);
        let thirds = Layout {
            version: 1,
            background: None,
            focus: 0,
            slots: [0.0, 1.0 / 3.0, 2.0 / 3.0]
                .iter()
                .map(|&x| Slot { rect: Rect { x, y: 0.0, w: 1.0 / 3.0, h: 1.0 } })
                .collect(),
        };
        let rects = thirds.resolve(853, 480);
        covers_exactly(&rects, 853, 480);
    }

    #[test]
    fn validate_rejects_bad_layouts() {
        let ok = presets::quadrants(4);
        assert!(ok.validate(4).is_ok());
        assert!(ok.validate(3).is_err());

        let mut wrong_version = ok.clone();
        wrong_version.version = 2;
        assert!(wrong_version.validate(4).is_err());

        let mut oversized = ok.clone();
        oversized.slots[1].rect.w = 0.7;
        assert!(oversized.validate(4).is_err());

        let mut zero = ok.clone();
        zero.slots[0].rect.h = 0.0;
        assert!(zero.validate(4).is_err());

        let mut negative = ok.clone();
        negative.slots[0].rect.x = -0.1;
        assert!(negative.validate(4).is_err());

        let mut bad_focus = ok.clone();
        bad_focus.focus = 4;
        assert!(bad_focus.validate(4).is_err());
    }

    #[test]
    fn layout_json_round_trip() {
        let layout = presets::three_player_l(0);
        let json = serde_json::to_string(&layout).unwrap();
        let back: Layout = serde_json::from_str(&json).unwrap();
        assert_eq!(layout, back);

        let minimal: Layout =
            serde_json::from_str(r#"{"slots":[{"rect":{"x":0,"y":0,"w":1,"h":1}}]}"#).unwrap();
        assert_eq!(minimal.version, 1);
        assert_eq!(minimal.focus, 0);
        assert!(minimal.background.is_none());
    }
}
