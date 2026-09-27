//! Privacy-permission status, and the deep links that take a user to the
//! system UI for fixing it.
//!
//! **Reading a status never prompts.** Prompting belongs to whichever process
//! owns the resource: the agent raises the Accessibility prompt because it owns
//! the event tap, and opens HID itself. A prompt from the wrong process records
//! the grant against the wrong code-signing identity (issue #214), so this
//! crate exposes only the non-prompting half plus [`open_pane`] — which is also
//! why no general-purpose macOS permission crate fits: they assume one app
//! asking for itself.
//!
//! Access is device-file permissions rather than consent dialogs: write to
//! `/dev/uinput` (the evdev/uinput hook's virtual devices) and read/write to
//! `/dev/hidraw*` (HID++ to the Bolt receiver or a direct connection). Both
//! come from the OpenLogi udev rules.

#[cfg(target_os = "linux")]
mod linux;

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests;

/// Tri-state result of a permission query.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PermissionStatus {
    /// The app may use the capability.
    Granted,
    /// The user denied it (or it's restricted).
    Denied,
    /// Not yet determined, or the platform can't report a definite state.
    Unknown,
}

/// A privacy permission with a platform action (deep-link or install guide).
#[derive(Clone, Copy)]
pub enum Permission {
    /// Input access for button remapping (the evdev/uinput hook).
    Accessibility,
}

#[cfg(target_os = "linux")]
pub use linux::input_device_access;

/// No-op: Linux has no pane to open — the udev-rules guide is shown inline in
/// the Settings window instead.
pub fn open_pane(_permission: Permission) {}
