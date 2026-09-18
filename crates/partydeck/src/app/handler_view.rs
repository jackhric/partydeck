use eframe::egui::ImageSource;

use super::icons::EXECUTABLE_ICON;
use crate::handler::Handler;

const CLAMP_CHARS: usize = 25;
const CLAMP_KEEP: usize = 22;

pub fn icon(h: &Handler) -> ImageSource<'_> {
    if h.path_handler.join("icon.png").exists() {
        format!("file://{}/icon.png", h.path_handler.display()).into()
    } else {
        EXECUTABLE_ICON
    }
}

/// Name shortened for the game list, cut on character boundaries.
pub fn display_clamp(h: &Handler) -> String {
    if h.name.chars().count() > CLAMP_CHARS {
        let head: String = h.name.chars().take(CLAMP_KEEP).collect();
        format!("{head}...")
    } else {
        h.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_is_char_safe() {
        let h = Handler {
            name: "ééééééééééééééééééééééééééééé".to_string(),
            ..Handler::default()
        };
        assert_eq!(display_clamp(&h), format!("{}...", "é".repeat(CLAMP_KEEP)));
        let short = Handler {
            name: "Short".to_string(),
            ..Handler::default()
        };
        assert_eq!(display_clamp(&short), "Short");
    }
}
