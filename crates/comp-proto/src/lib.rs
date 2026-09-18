//! Wire format shared by partydeck-comp and the partydeck launcher: layout
//! files, presets, control-socket commands and the session state document.

pub mod ipc;
pub mod layout;
pub mod presets;
pub mod state;

pub const PROTOCOL_VERSION: u32 = 1;
