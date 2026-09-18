/// Label the GUI shows for the guest entry in the profile dropdown.
pub const GUEST_LABEL: &str = "Guest";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileChoice {
    Guest,
    Named(String),
}

impl ProfileChoice {
    pub fn from_label(label: &str) -> Self {
        if label == GUEST_LABEL {
            ProfileChoice::Guest
        } else {
            ProfileChoice::Named(label.to_string())
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ProfileChoice::Guest => GUEST_LABEL,
            ProfileChoice::Named(name) => name,
        }
    }

    pub fn is_guest(&self) -> bool {
        matches!(self, ProfileChoice::Guest)
    }
}

#[derive(Clone, Debug)]
pub struct Instance {
    pub devices: Vec<usize>,
    /// Explicit XInput slot for this player's proxy pad; None derives it from devices.
    pub pad_slot: Option<u32>,
    pub profile: ProfileChoice,
    /// Resolved profile directory name; guests get a random ".Name".
    pub profname: String,
    pub width: u32,
    pub height: u32,
}

impl Instance {
    pub fn new(devices: Vec<usize>) -> Self {
        Instance {
            devices,
            pad_slot: None,
            profile: ProfileChoice::Guest,
            profname: String::new(),
            width: 0,
            height: 0,
        }
    }
}
