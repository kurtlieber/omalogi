//! Read-only installation ownership of the running GUI, not its download history.
//!
//! Detection runs once off the UI thread. A package receipt must identify this
//! executable, not merely another installed copy. Unmarked copies stay unknown.

use gpui::{App, Global};

#[cfg(any(target_os = "linux", test))]
mod linux;

/// Evidence-backed ownership of the currently running copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationSource {
    /// The package database owns this executable as part of `openlogi`.
    LinuxPackage(LinuxPackage),
    /// The resolved executable lives in the Nix store.
    Nix,
    /// No supported ownership evidence, unavailable metadata, or conflicting receipts.
    Unknown,
}

/// Linux package database that owns the running executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxPackage {
    /// Debian-family package registered with dpkg.
    Deb,
    /// RPM package registered with rpm.
    Rpm,
    /// Arch package registered with pacman.
    Arch,
}

/// Startup detection state; pending is distinct from an inconclusive result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installation {
    /// The one-time background probe has not completed yet.
    Detecting,
    /// The probe completed, possibly without conclusive ownership evidence.
    Detected(InstallationSource),
}

impl Global for Installation {}

impl InstallationSource {
    /// Inspect this process's executable. Does no network I/O or installation writes.
    pub fn detect() -> Self {
        let Ok(executable) = std::env::current_exe().and_then(std::fs::canonicalize) else {
            return Self::Unknown;
        };
        #[cfg(target_os = "linux")]
        return linux::detect(&executable);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = executable;
            Self::Unknown
        }
    }
}

/// Publish the installation state and start its one-time, off-main-thread probe.
pub fn install(cx: &mut App) {
    cx.set_global(Installation::Detecting);
    cx.spawn(async move |cx| {
        let source = cx
            .background_executor()
            .spawn(async { InstallationSource::detect() })
            .await;
        tracing::info!(?source, "detected installation source");
        cx.update(|cx| cx.set_global(Installation::Detected(source)));
    })
    .detach();
}
