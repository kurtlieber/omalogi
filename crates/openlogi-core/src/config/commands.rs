//! `[commands]`: user shell commands that replace a built-in action globally.
//!
//! An Omalogi addition (ADR-0005). Each key names a one-shot action and each
//! value is a shell string, so every binding of that action — on any device,
//! in any profile, on the keyboard page or in the Actions Ring — runs the
//! command instead of the built-in behaviour:
//!
//! ```toml
//! [commands]
//! OmarchyMenu = "~/bin/my-overview"
//! ToggleScratchpad = "hyprctl dispatch togglespecialworkspace term"
//! ```
//!
//! Keys are validated on load: an unknown action name, an action that cannot
//! be overridden, a duplicate (an upstream alias and its Omalogi name), or an
//! empty command fails the parse with the TOML location, rather than being
//! silently ignored.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, IntoDeserializer};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::binding::{Action, Effect};

/// The validated `[commands]` table, in the order the actions were written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandOverrides {
    entries: Vec<(Action, String)>,
}

impl CommandOverrides {
    /// Whether `action` may be overridden: a one-shot action with no payload
    /// that fires a shortcut, media key, or window-manager/power action.
    ///
    /// Clicks, scrolls and held chords need press/release pairing, and the
    /// DPI, SmartShift and Actions Ring actions run inside the agent, so none
    /// of those can be handed to a shell command.
    #[must_use]
    pub fn is_overridable(action: &Action) -> bool {
        matches!(
            action.effect(),
            Effect::Shortcut(_) | Effect::Media(_) | Effect::Native(_)
        )
    }

    /// Build from `(action, command)` pairs, rejecting what a config file
    /// would reject.
    ///
    /// # Errors
    ///
    /// Returns the message a config parse would report for the first invalid
    /// entry.
    pub fn new(entries: impl IntoIterator<Item = (Action, String)>) -> Result<Self, String> {
        let mut out = Self::default();
        for (action, command) in entries {
            out.insert(action, command)?;
        }
        Ok(out)
    }

    fn insert(&mut self, action: Action, command: String) -> Result<(), String> {
        if !Self::is_overridable(&action) {
            return Err(format!(
                "[commands] cannot override {}: only one-shot actions (shortcuts, media keys, \
                 navigation and system actions) can run a command",
                action.label()
            ));
        }
        if command.trim().is_empty() {
            return Err(format!(
                "[commands] {} has an empty command",
                action.label()
            ));
        }
        if self.get(&action).is_some() {
            return Err(format!(
                "[commands] {} is set twice (an upstream name and its Omalogi name both \
                 count)",
                action.label()
            ));
        }
        self.entries.push((action, command));
        Ok(())
    }

    /// The command that replaces `action`, if one is configured.
    #[must_use]
    pub fn get(&self, action: &Action) -> Option<&str> {
        self.entries
            .iter()
            .find(|(candidate, _)| candidate == action)
            .map(|(_, command)| command.as_str())
    }

    /// Whether no override is configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every configured `(action, command)` pair.
    pub fn iter(&self) -> impl Iterator<Item = (&Action, &str)> {
        self.entries
            .iter()
            .map(|(action, command)| (action, command.as_str()))
    }
}

impl Serialize for CommandOverrides {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;
        for (action, command) in &self.entries {
            map.serialize_entry(action, command)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for CommandOverrides {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = BTreeMap::<String, String>::deserialize(deserializer)?;
        let mut out = Self::default();
        for (name, command) in raw {
            let action = Action::deserialize(name.as_str().into_deserializer())
                .map_err(|_: de::value::Error| de::Error::custom(UnknownAction(&name)))?;
            out.insert(action, command).map_err(de::Error::custom)?;
        }
        Ok(out)
    }
}

struct UnknownAction<'a>(&'a str);

impl fmt::Display for UnknownAction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[commands] has no action named {:?}; use the name `config.toml` \
             bindings use, such as OmarchyMenu or VolumeUp",
            self.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, Serialize)]
    struct Doc {
        #[serde(default)]
        commands: CommandOverrides,
    }

    fn parse(src: &str) -> Result<CommandOverrides, String> {
        toml::from_str::<Doc>(src)
            .map(|doc| doc.commands)
            .map_err(|e| e.to_string())
    }

    #[test]
    fn overrides_load_by_action_name_and_round_trip() {
        let loaded =
            parse("[commands]\nOmarchyMenu = \"~/bin/overview\"\nVolumeUp = \"pamixer -i 2\"\n")
                .expect("valid");
        assert_eq!(loaded.get(&Action::OmarchyMenu), Some("~/bin/overview"));
        assert_eq!(loaded.get(&Action::VolumeUp), Some("pamixer -i 2"));
        assert_eq!(loaded.get(&Action::Copy), None);

        let saved = toml::to_string(&Doc {
            commands: loaded.clone(),
        })
        .expect("serialize");
        assert_eq!(parse(&saved).expect("reload"), loaded);
    }

    #[test]
    fn upstream_names_are_accepted_and_saved_under_the_omalogi_name() {
        let loaded = parse("[commands]\nMissionControl = \"x\"\n").expect("alias loads");
        assert_eq!(loaded.get(&Action::OmarchyMenu), Some("x"));
        let saved = toml::to_string(&Doc { commands: loaded }).expect("serialize");
        assert!(saved.contains("OmarchyMenu = \"x\""), "{saved}");
    }

    #[test]
    fn config_toml_carries_the_table() {
        let config: crate::config::Config =
            toml::from_str("schema_version = 7\n[commands]\nAppsMenu = \"wofi --show drun\"\n")
                .expect("config with [commands] parses");
        assert_eq!(
            config.commands.get(&Action::AppsMenu),
            Some("wofi --show drun")
        );
        let saved = toml::to_string(&crate::config::Config::default()).expect("serialize");
        assert!(
            !saved.contains("[commands]"),
            "empty table is omitted: {saved}"
        );
    }

    #[test]
    fn invalid_entries_fail_the_parse() {
        for (src, needle) in [
            ("[commands]\nNotAnAction = \"x\"\n", "no action named"),
            ("[commands]\nLeftClick = \"x\"\n", "cannot override"),
            ("[commands]\nScrollUp = \"x\"\n", "cannot override"),
            ("[commands]\nShowActionsRing = \"x\"\n", "cannot override"),
            ("[commands]\nToggleSmartShift = \"x\"\n", "cannot override"),
            ("[commands]\nNone = \"x\"\n", "cannot override"),
            ("[commands]\nCopy = \"  \"\n", "empty command"),
            (
                "[commands]\nShowDesktop = \"a\"\nToggleScratchpad = \"b\"\n",
                "set twice",
            ),
        ] {
            let error = parse(src).expect_err(src);
            assert!(error.contains(needle), "{src}: {error}");
        }
    }

    #[test]
    fn overridable_actions_are_the_one_shot_catalog_rows() {
        use crate::binding::Category;
        for action in Action::catalog() {
            let expected = match action.category() {
                Category::Editing | Category::Browser | Category::Media | Category::Navigation => {
                    true
                }
                Category::System => matches!(
                    action,
                    Action::LockScreen | Action::Screenshot | Action::CaptureRegion | Action::Sleep
                ),
                Category::Mouse | Category::Dpi | Category::Scroll => false,
            };
            assert_eq!(
                CommandOverrides::is_overridable(&action),
                expected,
                "{action:?}"
            );
        }
        assert!(!CommandOverrides::is_overridable(&Action::RunShellCommand(
            "x".into()
        )));
    }
}
