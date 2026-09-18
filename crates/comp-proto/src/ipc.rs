use serde::de::Deserializer;
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};

use crate::state::{SlotStatus, State};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    SetSlotStatus {
        slot: usize,
        status: SlotStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        logo: Option<String>,
    },
    Quit,
    GetState,
}

/// `Ok` and `Err` answer commands; `State` answers `get_state` with the bare
/// state document (no `ok` key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Ok,
    Err(String),
    State(State),
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
            Response::State(state) => state.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Response {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Ack {
                ok: bool,
                #[serde(default)]
                error: Option<String>,
            },
            State(State),
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Ack { ok: true, .. } => Response::Ok,
            Wire::Ack { ok: false, error } => Response::Err(error.unwrap_or_default()),
            Wire::State(state) => Response::State(state),
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
    use crate::state::tests::{FIXTURE_JSON, fixture};

    #[test]
    fn response_wire_format_is_exact() {
        assert_eq!(
            serde_json::to_string(&Response::Ok).unwrap(),
            r#"{"ok":true}"#
        );
        assert_eq!(
            serde_json::to_string(&Response::Err("boom".into())).unwrap(),
            r#"{"ok":false,"error":"boom"}"#
        );
        assert_eq!(
            serde_json::to_string(&Response::State(fixture())).unwrap(),
            FIXTURE_JSON
        );

        assert_eq!(
            serde_json::from_str::<Response>(r#"{"ok":true}"#).unwrap(),
            Response::Ok
        );
        assert_eq!(
            serde_json::from_str::<Response>(r#"{"ok":false,"error":"boom"}"#).unwrap(),
            Response::Err("boom".into())
        );
        assert_eq!(
            serde_json::from_str::<Response>(r#"{"ok":false}"#).unwrap(),
            Response::Err(String::new())
        );
        assert_eq!(
            serde_json::from_str::<Response>(FIXTURE_JSON).unwrap(),
            Response::State(fixture())
        );
        assert!(serde_json::from_str::<Response>(r#"{"nope":1}"#).is_err());
    }

    #[test]
    fn command_wire_format() {
        assert_eq!(
            serde_json::to_string(&Command::Quit).unwrap(),
            r#"{"cmd":"quit"}"#
        );
        assert_eq!(
            serde_json::to_string(&Command::GetState).unwrap(),
            r#"{"cmd":"get_state"}"#
        );
        assert_eq!(
            serde_json::to_string(&Command::SetSlotStatus {
                slot: 1,
                status: SlotStatus::Loading,
                label: None,
                avatar: None,
                logo: None
            })
            .unwrap(),
            r#"{"cmd":"set_slot_status","slot":1,"status":"loading"}"#
        );
        assert_eq!(
            decode_command(r#"{"cmd":"set_slot_status","slot":1,"status":"loading"}"#).unwrap(),
            Command::SetSlotStatus {
                slot: 1,
                status: SlotStatus::Loading,
                label: None,
                avatar: None,
                logo: None
            }
        );
        assert!(
            decode_command(r#"{"cmd":"set_slot_status","slot":1,"status":"crashed"}"#).is_err()
        );
        assert!(decode_command(r#"{"cmd":"ping"}"#).is_err());
        assert!(decode_command(r#"{"cmd":"set_focus","slot":0}"#).is_err());
    }

    #[test]
    fn command_round_trip() {
        let cmds = vec![
            Command::SetSlotStatus {
                slot: 0,
                status: SlotStatus::Running,
                label: Some("player 1".into()),
                avatar: Some("iVBORw0KGgo=".into()),
                logo: Some("bG9nbw==".into()),
            },
            Command::Quit,
            Command::GetState,
        ];
        for cmd in cmds {
            let line = encode(&cmd).unwrap();
            assert!(line.ends_with('\n'));
            assert_eq!(decode_command(&line).unwrap(), cmd);
        }
    }
}
