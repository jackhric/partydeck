pub const LAYOUT_PRESETS: &[&str] = &["auto", "horizontal", "vertical", "grid"];

pub const CHECK_FOR_UPDATES: &str = "DEFAULT: Enabled\n\nWARNING: CONTACTS GITHUB's SERVERS ON EVERY LAUNCH\nMakes partydeck check online for updates durring each launch, and notfies user when avaliable.";

pub const LAYOUT_PRESET: &str = "DEFAULT: auto\n\nHow instances are tiled on screen. auto/horizontal: 2 players stack above/below, 3-4 players use a grid. vertical: 2 players sit side by side. grid: always quadrants.";

pub const CONTROLLER_FILTER: &str = "DEFAULT: No Steam Input\n\nSelect which controllers to filter out. If you use Steam Input to remap controllers, you may want to select \"Only Steam Input\", but be warned that this option is experimental and is known to break certain Proton games.";

pub const PROFILE_UNIQUE_DIRS: &str = "DEFAULT: Enabled\n\nGives each profile their own data directories. For Windows games, this is the C:\\Users\\steamuser folder, for Linux native games this is the HOME directory. Note that disabling this means that PartyDeck instances may potentially modify your game's actual save data on disk.";

pub const ALLOW_SAME_DEVICE: &str = "DEFAULT: Disabled\n\nAllow multiple instances on the same device. This can be useful for testing or when one person wants to control multiple instances.";

pub const DISABLE_MOUNT_GAMEDIRS: &str = "DEFAULT: Disabled\n\nBy default, PartyDeck mounts game directories using fuse-overlayfs to let each instance write to the game's directory without conflicting with each other or affecting the game's installation. In addition, this lets handlers overlay content like mods or config files onto the game directory. Enabling this forces instances to launch from the original game directory without mounting, which will prevent handlers from using built-in mods, but may be useful for diagnosing issues.";

pub const PROTON_VERSION: &str = "DEFAULT: GE-Proton\n\nSpecify a Proton version. This can be a path, e.g. \"/path/to/proton\" or just a name, e.g. \"GE-Proton\" for the latest version of Proton-GE. If left blank, this will default to \"GE-Proton\". If unsure, leave this blank.";

pub const PROTON_SEPARATE_PFXS: &str = "DEFAULT: Enabled\n\nRuns each instance in separate Proton prefixes. If unsure, leave this checked. Multiple prefixes takes up more disk space, but generally provides better compatibility and fewer issues with Proton-based games.";

pub const PROTON_WOW64: &str = "DEFAULT: Enabled\n\nRuns Proton games in the new Wine WoW64 mode. If unsure, leave this checked.";

pub const ERASE_PREFIXES_CONFIRM: &str = "This will erase all Proton prefixes used by PartyDeck. This shouldn't erase profile/game-specific data, but exercise caution. Are you sure?";

pub const GAMESCOPE_FIX_LOWRES: &str = "Many games have graphical problems or even crash when running at resolutions below 600p. If this is enabled, any instances below 600p will automatically be resized before launching.";

pub const KBM_SUPPORT: &str = "Runs a custom Gamescope build with support for holding keyboards and mice. If you want to use your own Gamescope installation, uncheck this.";

pub const GAMESCOPE_FORCE_GRAB_CURSOR: &str = "Sets the \"--force-grab-cursor\" flag in Gamescope. This keeps the cursor within the Gamescope window. If unsure, leave this unchecked.";

pub const PROFILES_INFO: &str =
    "Create profiles to persistently store game save data, settings, and stats.";

pub const HIDRAW_LABEL: &str = "Enable HIDraw for non-Xbox controllers (fixes Unity Input System games; may cause double input in non-Unity games!)";

pub const CONTROLLER_MAPPINGS: &str = "Some native Linux games run using an older version of SDL2 that doesn't support newer controllers; you can edit the handler and change the SDL2 Override setting to \"Steam Runtime\" for older 32-bit games, or \"System Installation\" for 64-bit games.\n\nWindows Unity-based games may not recognize input from PlayStation controllers; the current workaround for this is to use them through Steam Input, and change PartyDeck controller filter setting to \"Only Steam Input\".";

pub const INPUT_GROUP_COMMAND: &str = "sudo usermod -aG input $USER";

pub const HANDLER_OLDER_HINT: &str =
    "Up-to-date handlers can be found by clicking the ⮋ button on the top bar of the launcher.";
pub const HANDLER_NEWER_HINT: &str = "It is recommended to update PartyDeck to the latest version.";

pub fn handler_version_mismatch(handler_is_older: bool) -> String {
    let (age, hint) = if handler_is_older {
        ("an older", HANDLER_OLDER_HINT)
    } else {
        ("a newer", HANDLER_NEWER_HINT)
    };
    format!(
        "This handler was meant for use with {age} version of PartyDeck; you may experience issues or the game may not work at all. {hint} If everything still works fine, you can prevent this message appearing in the future by editing the handler, updating the spec version and saving."
    )
}
