use std::path::PathBuf;
use std::process::{Command, Output};

fn scratch_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("partydeck-cli-{}-{name}", std::process::id()));
    std::fs::create_dir_all(dir.join("share")).unwrap();
    dir
}

fn partydeck(home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_partydeck"))
        .args(args)
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join("share"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .expect("failed to run partydeck binary")
}

#[test]
fn help_exits_zero_without_a_display() {
    let home = scratch_home("help");
    let out = partydeck(&home, &["--help"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--fullscreen"));
    assert!(!text.contains("--kwin"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn config_show_prints_default_json() {
    let home = scratch_home("config");
    let out = partydeck(&home, &["config", "show"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    assert_eq!(json["layout_preset"], "auto");
    assert_eq!(json["pad_filter_type"], "NoSteamInput");
    assert_eq!(json["kbm_support"], true);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn config_set_json_round_trips() {
    let home = scratch_home("set");
    std::fs::create_dir_all(home.join("share/partydeck")).unwrap();
    let out = partydeck(
        &home,
        &[
            "config",
            "set-json",
            r#"{"layout_preset":"grid","proxy_gamepads":false}"#,
        ],
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = partydeck(&home, &["config", "show"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    assert_eq!(json["layout_preset"], "grid");
    assert_eq!(json["proxy_gamepads"], false);
    assert_eq!(json["kbm_support"], true);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn profile_list_is_empty_json_array_on_fresh_data_dir() {
    let home = scratch_home("profiles");
    let out = partydeck(&home, &["profile", "list"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    assert_eq!(json, serde_json::json!([]));
    let _ = std::fs::remove_dir_all(home);
}
