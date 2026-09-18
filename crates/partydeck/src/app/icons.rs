use eframe::egui::{ImageSource, include_image};

pub const BTN_SOUTH: ImageSource<'static> = include_image!("../../assets/glyphs/BTN_SOUTH.png");
pub const BTN_EAST: ImageSource<'static> = include_image!("../../assets/glyphs/BTN_EAST.png");
pub const BTN_NORTH: ImageSource<'static> = include_image!("../../assets/glyphs/BTN_NORTH.png");
pub const BTN_WEST: ImageSource<'static> = include_image!("../../assets/glyphs/BTN_WEST.png");
pub const BTN_START: ImageSource<'static> = include_image!("../../assets/glyphs/BTN_START.png");
pub const MOUSE_RIGHT: ImageSource<'static> = include_image!("../../assets/glyphs/MOUSE_RIGHT.png");

pub const EXECUTABLE_ICON: ImageSource<'static> =
    include_image!("../../assets/icons/executable_icon.png");

pub const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../assets/icons/icon.png");
