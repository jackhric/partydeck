use super::profile_dir;
use crate::error::Result;
use crate::fs::copy_dir_recursive;
use crate::handler::Handler;

/// Creates the per-profile save folder for a handler and seeds it from the
/// handler's `profile_copy_*` templates.
pub fn create_profile_gamesave(name: &str, h: &Handler) -> Result<()> {
    let uid = h.handler_dir_name();
    let path_prof = profile_dir(name);
    let path_gamesave = path_prof.join("gamesaves").join(uid);

    if path_gamesave.exists() {
        return Ok(());
    }
    eprintln!("[partydeck] Creating game save {uid} for {name}");
    std::fs::create_dir_all(&path_gamesave)?;

    if let Some(appid) = h.steam_appid
        && h.use_goldberg
    {
        let path_exec = path_gamesave.join(&h.exec);
        let path_execdir = path_exec.parent().ok_or("couldn't get parent")?;
        std::fs::create_dir_all(path_execdir)?;
        std::fs::write(path_execdir.join("steam_appid.txt"), appid.to_string())?;
    }

    let templates = [
        ("profile_copy_gamesave", path_gamesave),
        ("profile_copy_home", path_prof.join("home")),
        ("profile_copy_windata", path_prof.join("windata")),
    ];
    for (template, dest) in templates {
        let src = h.path_handler.join(template);
        if src.exists() {
            copy_dir_recursive(&src, &dest)?;
        }
    }
    Ok(())
}
