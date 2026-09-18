// Non-Steam shortcuts report a 64-bit SteamGameId with the 32-bit shortcut
// appid in the upper half; real Steam apps report the appid directly.
fn appid_from_gameid(gameid: u64) -> u32 {
    if gameid > u32::MAX as u64 {
        (gameid >> 32) as u32
    } else {
        gameid as u32
    }
}

pub fn app_id() -> Option<String> {
    let appid = std::env::var("SteamAppId")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&id| id != 0)
        .or_else(|| {
            std::env::var("SteamGameId")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .map(appid_from_gameid)
                .filter(|&id| id != 0)
        })?;
    Some(format!("steam_app_{appid}"))
}

#[cfg(test)]
mod tests {
    use super::appid_from_gameid;

    #[test]
    fn gameid_to_appid() {
        assert_eq!(
            appid_from_gameid((2488242132u64 << 32) | (1 << 25)),
            2488242132
        );
        assert_eq!(appid_from_gameid(620), 620);
    }
}
