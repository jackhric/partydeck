const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/wunnr/partydeck/releases/latest";

/// True when the newest GitHub release tag differs from this build's version.
/// Any network or parse failure reads as "no update".
pub fn check_for_partydeck_update() -> bool {
    let Ok(response) = reqwest::blocking::Client::new()
        .get(LATEST_RELEASE_URL)
        .header("User-Agent", "partydeck")
        .send()
    else {
        return false;
    };
    let Ok(release) = response.json::<serde_json::Value>() else {
        return false;
    };
    let Some(tag_name) = release["tag_name"].as_str() else {
        return false;
    };
    let latest_version = tag_name.strip_prefix('v').unwrap_or(tag_name);
    latest_version != env!("CARGO_PKG_VERSION")
}
