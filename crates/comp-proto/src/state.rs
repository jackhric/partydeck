use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
pub use crate::layout::PixelRect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "proto.ts"))]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "proto.ts"))]
#[serde(rename_all = "snake_case")]
pub enum SlotStatus {
    Loading,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "proto.ts"))]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    Off,
    #[default]
    Faint,
    Medium,
    Strong,
}

impl BorderStyle {
    pub const ALL: [BorderStyle; 4] = [
        BorderStyle::Off,
        BorderStyle::Faint,
        BorderStyle::Medium,
        BorderStyle::Strong,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            BorderStyle::Off => "off",
            BorderStyle::Faint => "faint",
            BorderStyle::Medium => "medium",
            BorderStyle::Strong => "strong",
        }
    }
}

impl fmt::Display for BorderStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BorderStyle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        Self::ALL
            .into_iter()
            .find(|b| b.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown border style {s:?} (off, faint, medium, strong)"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "proto.ts"))]
pub struct SlotState {
    pub rect: PixelRect,
    pub live: bool,
    pub status: Option<SlotStatus>,
    pub label: Option<String>,
    pub avatar: Option<String>,
    pub logo: Option<String>,
    pub controller_disconnected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export_to = "proto.ts"))]
pub struct State {
    #[serde(default = "protocol_version")]
    pub proto: u32,
    pub size: Size,
    pub focus: usize,
    pub border: BorderStyle,
    pub slots: Vec<SlotState>,
}

fn protocol_version() -> u32 {
    PROTOCOL_VERSION
}

impl State {
    pub fn new(size: Size, focus: usize, border: BorderStyle, slots: Vec<SlotState>) -> Self {
        State {
            proto: PROTOCOL_VERSION,
            size,
            focus,
            border,
            slots,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn fixture() -> State {
        State::new(
            Size { w: 1280, h: 800 },
            1,
            BorderStyle::Medium,
            vec![
                SlotState {
                    rect: PixelRect {
                        x: 0,
                        y: 0,
                        w: 640,
                        h: 800,
                    },
                    live: true,
                    status: Some(SlotStatus::Loading),
                    label: Some("player 1".into()),
                    avatar: Some("iVBORw0KGgo=".into()),
                    logo: None,
                    controller_disconnected: false,
                },
                SlotState {
                    rect: PixelRect {
                        x: 640,
                        y: 0,
                        w: 640,
                        h: 800,
                    },
                    live: false,
                    status: None,
                    label: None,
                    avatar: None,
                    logo: None,
                    controller_disconnected: false,
                },
            ],
        )
    }

    pub(crate) const FIXTURE_JSON: &str = concat!(
        r#"{"proto":1,"size":{"w":1280,"h":800},"focus":1,"border":"medium","slots":["#,
        r#"{"rect":{"x":0,"y":0,"w":640,"h":800},"live":true,"status":"loading","label":"player 1","avatar":"iVBORw0KGgo=","logo":null,"controller_disconnected":false},"#,
        r#"{"rect":{"x":640,"y":0,"w":640,"h":800},"live":false,"status":null,"label":null,"avatar":null,"logo":null,"controller_disconnected":false}"#,
        r#"]}"#
    );

    #[test]
    fn state_wire_format_is_exact() {
        assert_eq!(serde_json::to_string(&fixture()).unwrap(), FIXTURE_JSON);
        assert_eq!(
            serde_json::from_str::<State>(FIXTURE_JSON).unwrap(),
            fixture()
        );
    }

    #[test]
    fn state_without_proto_field_defaults_to_current_version() {
        let json = FIXTURE_JSON.replacen(r#""proto":1,"#, "", 1);
        assert_eq!(serde_json::from_str::<State>(&json).unwrap(), fixture());
    }

    #[test]
    fn slot_status_wire_strings() {
        assert_eq!(
            serde_json::to_string(&SlotStatus::Loading).unwrap(),
            r#""loading""#
        );
        assert_eq!(
            serde_json::to_string(&SlotStatus::Running).unwrap(),
            r#""running""#
        );
        assert_eq!(
            serde_json::from_str::<SlotStatus>(r#""running""#).unwrap(),
            SlotStatus::Running
        );
        assert!(serde_json::from_str::<SlotStatus>(r#""Loading""#).is_err());
    }

    #[test]
    fn border_style_parse_display_and_wire() {
        for style in BorderStyle::ALL {
            assert_eq!(style.to_string().parse::<BorderStyle>().unwrap(), style);
            assert_eq!(
                serde_json::to_string(&style).unwrap(),
                format!("\"{style}\""),
                "serde and Display must agree"
            );
        }
        assert_eq!("faint".parse::<BorderStyle>().unwrap(), BorderStyle::Faint);
        assert_eq!(
            " Strong ".parse::<BorderStyle>().unwrap(),
            BorderStyle::Strong
        );
        assert!("bold".parse::<BorderStyle>().is_err());
        assert_eq!(BorderStyle::default(), BorderStyle::Faint);
    }

    #[cfg(feature = "ts")]
    #[test]
    fn export_typescript_bindings() {
        use ts_rs::TS;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../overlay/ui/src/generated");
        let cfg = ts_rs::Config::new().with_out_dir(&dir);
        State::export_all(&cfg).unwrap();
        let out = std::fs::read_to_string(dir.join("proto.ts")).unwrap();
        for name in [
            "State",
            "SlotState",
            "SlotStatus",
            "BorderStyle",
            "PixelRect",
            "Size",
        ] {
            assert!(
                out.contains(&format!("export type {name} ")),
                "{name} missing from proto.ts"
            );
        }
    }
}
