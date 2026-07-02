use serde::de::Deserializer;
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};

use crate::layout::Layout;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    SetLayout {
        layout: Layout,
    },
    SetFocus {
        slot: usize,
    },
    SetSlotStatus {
        slot: usize,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Ping,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    Ok,
    Err(String),
}

impl Serialize for Response {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Response::Ok => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("ok", &true)?;
                map.end()
            }
            Response::Err(error) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("ok", &false)?;
                map.serialize_entry("error", error)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Response {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            ok: bool,
            #[serde(default)]
            error: Option<String>,
        }
        let wire = Wire::deserialize(deserializer)?;
        Ok(if wire.ok {
            Response::Ok
        } else {
            Response::Err(wire.error.unwrap_or_default())
        })
    }
}

pub fn encode<T: Serialize>(msg: &T) -> serde_json::Result<String> {
    Ok(format!("{}\n", serde_json::to_string(msg)?))
}

pub fn decode_command(line: &str) -> serde_json::Result<Command> {
    serde_json::from_str(line.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[test]
    fn response_wire_format_is_exact() {
        assert_eq!(serde_json::to_string(&Response::Ok).unwrap(), r#"{"ok":true}"#);
        assert_eq!(
            serde_json::to_string(&Response::Err("boom".into())).unwrap(),
            r#"{"ok":false,"error":"boom"}"#
        );
        assert_eq!(
            serde_json::from_str::<Response>(r#"{"ok":true}"#).unwrap(),
            Response::Ok
        );
        assert_eq!(
            serde_json::from_str::<Response>(r#"{"ok":false,"error":"boom"}"#).unwrap(),
            Response::Err("boom".into())
        );
    }

    #[test]
    fn command_wire_format() {
        assert_eq!(
            serde_json::to_string(&Command::SetFocus { slot: 2 }).unwrap(),
            r#"{"cmd":"set_focus","slot":2}"#
        );
        assert_eq!(serde_json::to_string(&Command::Ping).unwrap(), r#"{"cmd":"ping"}"#);
        assert_eq!(
            decode_command(r#"{"cmd":"set_slot_status","slot":1,"status":"loading"}"#).unwrap(),
            Command::SetSlotStatus { slot: 1, status: "loading".into(), label: None }
        );
    }

    #[test]
    fn command_round_trip() {
        let cmds = vec![
            Command::SetLayout { layout: presets::quadrants(4) },
            Command::SetFocus { slot: 1 },
            Command::SetSlotStatus { slot: 0, status: "loading".into(), label: Some("player 1".into()) },
            Command::Ping,
            Command::Quit,
        ];
        for cmd in cmds {
            let line = encode(&cmd).unwrap();
            assert!(line.ends_with('\n'));
            assert_eq!(decode_command(&line).unwrap(), cmd);
        }
    }
}
