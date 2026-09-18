use std::io;
use std::path::{Path, PathBuf};

use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::error::Result;

pub fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    for entry in walk_dir(src)? {
        let new_path = dest.join(entry.strip_prefix(src)?);

        if entry.is_symlink() {
            let target = std::fs::read_link(&entry)?;
            std::os::unix::fs::symlink(target, new_path)?;
        } else if entry.is_dir() {
            std::fs::create_dir_all(&new_path)?;
        } else {
            if let Some(parent) = new_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if new_path.exists() {
                std::fs::remove_file(&new_path)?;
            }
            std::fs::copy(&entry, &new_path)?;
        }
    }
    Ok(())
}

fn walk_dir(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(path)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() && !path.is_symlink() {
            paths.push(path.clone());
            paths.extend(walk_dir(&path)?);
        } else {
            paths.push(path);
        }
    }
    Ok(paths)
}

pub fn zip_dir(src_dir: &Path, dest: &Path) -> Result<()> {
    let mut zip = ZipWriter::new(std::fs::File::create(dest)?);
    let options = SimpleFileOptions::default();

    for entry in walk_dir(src_dir)? {
        let name = entry.strip_prefix(src_dir)?;
        if entry.is_symlink() {
            zip.add_symlink_from_path(name, std::fs::read_link(&entry)?, options)?;
        } else if entry.is_dir() {
            zip.add_directory_from_path(name, options)?;
        } else {
            zip.start_file_from_path(name, options)?;
            io::copy(&mut std::fs::File::open(&entry)?, &mut zip)?;
        }
    }

    zip.finish()?;
    Ok(())
}
