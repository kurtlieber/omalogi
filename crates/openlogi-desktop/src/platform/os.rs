//! Best-effort host OS version string for the diagnostics report, plus syncing
//! the native window chrome (titlebar) to the in-app appearance preference.

use openlogi_core::config::Appearance;

/// The OS product version from `/etc/os-release` (`PRETTY_NAME`), or `None`
/// when unavailable.
#[must_use]
pub fn os_version() -> Option<String> {
    let release = std::fs::read_to_string("/etc/os-release").ok()?;
    release.lines().find_map(|line| {
        let value = line.strip_prefix("PRETTY_NAME=")?.trim_matches('"');
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Linux window chrome follows the client-side titlebar, which already tracks
/// the resolved theme; nothing to sync natively.
pub fn set_app_appearance(_appearance: Appearance) {}
