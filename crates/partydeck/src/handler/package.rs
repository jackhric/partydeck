use std::path::Path;

use super::{Handler, unique_handler_dir};
use crate::error::Result;
use crate::fs::{copy_dir_recursive, zip_dir};
use crate::launch::clear_tmp;
use crate::paths::tmp_dir;

const PACKAGE_EXT: &str = "pd2";

/// Zips the handler directory to `dest` (given a `.pd2` extension if missing)
/// with `path_gameroot` cleared so importers set their own.
pub fn export_pd2(h: &Handler, dest: &Path) -> Result<()> {
    if h.name.is_empty() {
        return Err("Name cannot be empty".into());
    }
    let mut dest = dest.to_path_buf();
    if dest.extension() != Some(PACKAGE_EXT.as_ref()) {
        dest.set_extension(PACKAGE_EXT);
    }

    let tmpdir = tmp_dir();
    std::fs::create_dir_all(&tmpdir)?;
    copy_dir_recursive(&h.path_handler, &tmpdir)?;

    let mut exported = h.clone();
    exported.path_gameroot = String::new();
    std::fs::write(
        tmpdir.join("handler.json"),
        serde_json::to_string_pretty(&exported)?,
    )?;

    if dest.is_file() {
        std::fs::remove_file(&dest)?;
    }
    zip_dir(&tmpdir, &dest)?;
    clear_tmp()?;
    Ok(())
}

/// Extracts a `.pd2` archive into a fresh handler directory named after the file.
pub fn import_pd2(file: &Path) -> Result<()> {
    if !file.is_file() || file.extension() != Some(PACKAGE_EXT.as_ref()) {
        return Err("Handler not valid!".into());
    }
    let name = file
        .file_stem()
        .ok_or("No filename")?
        .to_string_lossy()
        .to_string();

    let dir_tmp = tmp_dir();
    std::fs::create_dir_all(&dir_tmp)?;
    let mut archive = zip::ZipArchive::new(std::fs::File::open(file)?)?;
    archive.extract(&dir_tmp)?;

    if !dir_tmp.join("handler.json").exists() {
        clear_tmp()?;
        return Err("handler.json not found in archive".into());
    }

    copy_dir_recursive(&dir_tmp, &unique_handler_dir(&name))?;
    clear_tmp()?;
    Ok(())
}
