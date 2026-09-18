use crate::handler::{Handler, scan_handlers};
use crate::instance::{GUEST_LABEL, Instance, ProfileChoice};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuPage {
    Home,
    Settings,
    Profiles,
    EditHandler,
    Game,
    Instances,
}

impl MenuPage {
    pub fn has_info_panel(self) -> bool {
        !matches!(self, MenuPage::Home | MenuPage::Instances)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsPage {
    General,
    Proton,
    Gamescope,
}

/// Saved handlers shown in the games panel plus the highlighted entry.
pub struct HandlerLibrary {
    pub list: Vec<Handler>,
    pub selected: usize,
}

impl HandlerLibrary {
    pub fn new(list: Vec<Handler>) -> Self {
        let mut lib = HandlerLibrary { list, selected: 0 };
        lib.clamp_selected();
        lib
    }

    pub fn rescan(&mut self) {
        self.replace(scan_handlers());
    }

    pub fn replace(&mut self, list: Vec<Handler>) {
        self.list = list;
        self.clamp_selected();
    }

    fn clamp_selected(&mut self) {
        if self.selected >= self.list.len() {
            self.selected = 0;
        }
    }

    pub fn current(&self) -> Option<&Handler> {
        self.list.get(self.selected)
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

/// Instances being assembled on the Instances page before a launch.
#[derive(Default, Debug)]
pub struct SessionDraft {
    pub instances: Vec<Instance>,
    /// Instance that receives the next added device instead of a new instance.
    pub adding_device_to: Option<usize>,
}

/// Dropdown entries of the Instances page: the guest entry followed by the
/// profile names. Indices are only meaningful against the list they came from.
#[derive(Default, Debug)]
pub struct ProfileChoices {
    labels: Vec<String>,
}

impl ProfileChoices {
    pub fn from_names(names: &[String]) -> Self {
        let labels = std::iter::once(GUEST_LABEL.to_string())
            .chain(names.iter().cloned())
            .collect();
        ProfileChoices { labels }
    }

    pub fn len(&self) -> usize {
        self.labels.len()
    }

    pub fn label(&self, index: usize) -> String {
        self.labels.get(index).cloned().unwrap_or_default()
    }

    pub fn index_of(&self, choice: &ProfileChoice) -> usize {
        self.labels
            .iter()
            .position(|label| label == choice.label())
            .unwrap_or(0)
    }

    pub fn choice_at(&self, index: usize) -> ProfileChoice {
        self.labels
            .get(index)
            .map_or(ProfileChoice::Guest, |label| {
                ProfileChoice::from_label(label)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handlers(n: usize) -> Vec<Handler> {
        (0..n)
            .map(|i| Handler {
                name: format!("Game {i}"),
                ..Handler::default()
            })
            .collect()
    }

    #[test]
    fn selected_handler_is_clamped_after_replace() {
        let mut lib = HandlerLibrary::new(handlers(3));
        lib.selected = 2;
        lib.replace(handlers(2));
        assert_eq!(lib.selected, 0);
        lib.selected = 1;
        lib.replace(handlers(2));
        assert_eq!(lib.selected, 1);
        lib.replace(Vec::new());
        assert_eq!(lib.selected, 0);
        assert!(lib.current().is_none());
    }

    #[test]
    fn current_handler_follows_selection() {
        let mut lib = HandlerLibrary::new(handlers(2));
        lib.selected = 1;
        assert_eq!(lib.current().map(|h| h.name.as_str()), Some("Game 1"));
    }

    #[test]
    fn profile_choices_round_trip() {
        let names = vec!["Alice".to_string(), "Bob".to_string()];
        let choices = ProfileChoices::from_names(&names);
        assert_eq!(choices.len(), 3);
        assert_eq!(choices.label(0), GUEST_LABEL);
        assert_eq!(choices.index_of(&ProfileChoice::Guest), 0);
        assert_eq!(choices.index_of(&ProfileChoice::Named("Bob".into())), 2);
        assert_eq!(choices.choice_at(2), ProfileChoice::Named("Bob".into()));
        assert_eq!(choices.choice_at(0), ProfileChoice::Guest);
    }

    #[test]
    fn unknown_profile_and_index_fall_back_to_guest() {
        let choices = ProfileChoices::from_names(&["Alice".to_string()]);
        assert_eq!(choices.index_of(&ProfileChoice::Named("Zed".into())), 0);
        assert_eq!(choices.choice_at(7), ProfileChoice::Guest);
        assert_eq!(choices.label(7), "");
        let empty = ProfileChoices::default();
        assert_eq!(empty.choice_at(0), ProfileChoice::Guest);
    }
}
