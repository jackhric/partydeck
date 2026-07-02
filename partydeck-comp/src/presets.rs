use crate::layout::{Layout, Rect, Slot};

fn from_rects(rects: Vec<Rect>) -> Layout {
    Layout {
        version: 1,
        background: None,
        focus: 0,
        slots: rects.into_iter().map(|rect| Slot { rect }).collect(),
    }
}

pub fn fullscreen() -> Layout {
    from_rects(vec![Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }])
}

pub fn halves_h() -> Layout {
    from_rects(vec![
        Rect { x: 0.0, y: 0.0, w: 1.0, h: 0.5 },
        Rect { x: 0.0, y: 0.5, w: 1.0, h: 0.5 },
    ])
}

pub fn halves_v() -> Layout {
    from_rects(vec![
        Rect { x: 0.0, y: 0.0, w: 0.5, h: 1.0 },
        Rect { x: 0.5, y: 0.0, w: 0.5, h: 1.0 },
    ])
}

pub fn quadrants(players: usize) -> Layout {
    match players {
        0 | 1 => fullscreen(),
        2 => halves_h(),
        n => {
            let quarters = [
                Rect { x: 0.0, y: 0.0, w: 0.5, h: 0.5 },
                Rect { x: 0.5, y: 0.0, w: 0.5, h: 0.5 },
                Rect { x: 0.0, y: 0.5, w: 0.5, h: 0.5 },
                Rect { x: 0.5, y: 0.5, w: 0.5, h: 0.5 },
            ];
            from_rects(quarters[..n.min(4)].to_vec())
        }
    }
}

pub fn three_player_l(big: usize) -> Layout {
    let big = big.min(2);
    let smalls = [
        Rect { x: 0.5, y: 0.0, w: 0.5, h: 0.5 },
        Rect { x: 0.5, y: 0.5, w: 0.5, h: 0.5 },
    ];
    let mut rects = Vec::with_capacity(3);
    let mut small = smalls.iter();
    for i in 0..3 {
        if i == big {
            rects.push(Rect { x: 0.0, y: 0.0, w: 0.5, h: 1.0 });
        } else {
            rects.push(*small.next().unwrap());
        }
    }
    from_rects(rects)
}

pub fn priority(main: usize, players: usize) -> Layout {
    if players <= 1 {
        return fullscreen();
    }
    let main = main.min(players - 1);
    let side_h = 1.0 / (players - 1) as f32;
    let mut rects = Vec::with_capacity(players);
    let mut side = 0;
    for i in 0..players {
        if i == main {
            rects.push(Rect { x: 0.0, y: 0.0, w: 0.7, h: 1.0 });
        } else {
            rects.push(Rect { x: 0.7, y: side as f32 * side_h, w: 0.3, h: side_h });
            side += 1;
        }
    }
    from_rects(rects)
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
            assert!(priority(0, n).validate(n).is_ok(), "priority(0, {n})");
        }
        for big in 0..3 {
            assert!(three_player_l(big).validate(3).is_ok(), "three_player_l({big})");
        }
    }

    #[test]
    fn quadrants_two_is_stacked() {
        assert_eq!(quadrants(2), halves_h());
    }

    #[test]
    fn three_player_l_places_big_slot() {
        let layout = three_player_l(1);
        assert_eq!(layout.slots[1].rect, Rect { x: 0.0, y: 0.0, w: 0.5, h: 1.0 });
        assert_eq!(layout.slots[0].rect, Rect { x: 0.5, y: 0.0, w: 0.5, h: 0.5 });
    }
}
