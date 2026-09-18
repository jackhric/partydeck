use std::path::{Path, PathBuf};

use dialog::{Choice, DialogBox};
use rfd::FileDialog;

use crate::error::Result;
use crate::paths::PATH_HOME;

const PD2_FILTER: (&str, &[&str]) = ("PartyDeck Handler Package", &["pd2"]);

pub fn msg(title: &str, contents: &str) {
    let _ = dialog::Message::new(contents).title(title).show();
}

/// Opens `path` in the desktop file manager; shows `error` when that fails.
pub fn open_in_file_manager(path: &Path, error: &str) {
    if std::process::Command::new("xdg-open")
        .arg(path)
        .status()
        .is_err()
    {
        msg("Error", error);
    }
}

pub fn yesno(title: &str, contents: &str) -> bool {
    dialog::Question::new(contents)
        .title(title)
        .show()
        .is_ok_and(|choice| choice == Choice::Yes)
}

/// Text prompt; None when cancelled or when no dialog backend is available.
pub fn prompt_text(title: &str, label: &str) -> Option<String> {
    match dialog::Input::new(label).title(title).show() {
        Ok(answer) => answer,
        Err(e) => {
            eprintln!("[partydeck] Could not display dialog box: {e}");
            None
        }
    }
}

pub fn dir_dialog() -> Result<PathBuf> {
    FileDialog::new()
        .set_title("Select Folder")
        .set_directory(&*PATH_HOME)
        .pick_folder()
        .ok_or_else(|| "No folder selected".into())
}

/// Picks a file under `base_dir` and returns its path relative to it.
pub fn file_dialog_relative(base_dir: &Path) -> Result<PathBuf> {
    let file = FileDialog::new()
        .set_title("Select File")
        .set_directory(base_dir)
        .pick_file()
        .ok_or("No file selected")?;
    let relative = file
        .strip_prefix(base_dir)
        .map_err(|_| "Selected file is not within the base directory")?;
    Ok(relative.to_path_buf())
}

pub fn pick_png(title: &str) -> Option<PathBuf> {
    FileDialog::new()
        .set_title(title)
        .set_directory(&*PATH_HOME)
        .add_filter("PNG Image", &["png"])
        .pick_file()
        .filter(|file| file.extension().is_some_and(|ext| ext == "png"))
}

pub fn pick_pd2_to_import() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select File")
        .set_directory(&*PATH_HOME)
        .add_filter(PD2_FILTER.0, PD2_FILTER.1)
        .pick_file()
}

pub fn pick_pd2_save_path() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Save file to:")
        .set_directory(&*PATH_HOME)
        .add_filter(PD2_FILTER.0, PD2_FILTER.1)
        .save_file()
}
