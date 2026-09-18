use crate::layout::{Layout, Rect};

pub fn fullscreen() -> Layout {
    Layout::from_rects(vec![Rect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    }])
}

pub fn halves_h() -> Layout {
    Layout::from_rects(vec![
        Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 0.5,
        },
        Rect {
            x: 0.0,
            y: 0.5,
            w: 1.0,
            h: 0.5,
        },
    ])
}

pub fn halves_v() -> Layout {
    Layout::from_rects(vec![
        Rect {
            x: 0.0,
            y: 0.0,
            w: 0.5,
            h: 1.0,
        },
        Rect {
            x: 0.5,
            y: 0.0,
            w: 0.5,
            h: 1.0,
        },
    ])
}

pub fn quadrants(players: usize) -> Layout {
    match players {
        0 | 1 => fullscreen(),
        2 => halves_h(),
        n => {
            let quarters = [
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.5,
                    h: 0.5,
                },
                Rect {
                    x: 0.5,
                    y: 0.0,
                    w: 0.5,
                    h: 0.5,
                },
                Rect {
                    x: 0.0,
                    y: 0.5,
                    w: 0.5,
                    h: 0.5,
                },
                Rect {
                    x: 0.5,
                    y: 0.5,
                    w: 0.5,
                    h: 0.5,
                },
            ];
            Layout::from_rects(quarters[..n.min(4)].to_vec())
        }
    }
}

pub fn by_name(name: &str, players: usize) -> Option<Layout> {
    match name {
        "vertical" => Some(match players {
            0 | 1 => fullscreen(),
            2 => halves_v(),
            n => quadrants(n),
        }),
        "auto" | "horizontal" | "grid" => Some(quadrants(players)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_validate() {
        assert!(fullscreen().validate(1).is_ok());
        assert!(halves_h().validate(2).is_ok());
        assert!(halves_v().validate(2).is_ok());
        for n in 2..=4 {
            assert!(quadrants(n).validate(n).is_ok(), "quadrants({n})");
        }
    }

    #[test]
    fn by_name_resolves_presets() {
        assert_eq!(by_name("auto", 2), Some(quadrants(2)));
        assert_eq!(by_name("vertical", 2), Some(halves_v()));
        assert_eq!(by_name("grid", 4), Some(quadrants(4)));
        assert_eq!(by_name("bogus", 2), None);
    }

    #[test]
    fn quadrants_two_is_stacked() {
        assert_eq!(quadrants(2), halves_h());
    }
}
