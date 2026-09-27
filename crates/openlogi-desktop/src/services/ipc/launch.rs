//! Bringing the agent up when the socket is unreachable: launch the agent
//! binary that sits next to the GUI directly. The agent's own systemd user
//! unit handles login start and crash respawn.

use std::path::PathBuf;

use tracing::{info, warn};

/// Set when the agent announced a deliberate suite shutdown (the
/// `openlogi://quit` deep link arrives before the agent exits) — the
/// unreachable→spawn reflex has been observed resurrecting the agent seconds
/// after Quit. Never cleared: this process is quitting too.
static SUITE_QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Record that the user quit the whole suite, so the
/// IPC client stops respawning the agent during this GUI's teardown.
pub fn mark_suite_quitting() {
    SUITE_QUITTING.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Launch the agent once when the socket is unreachable. Detached so it
/// outlives the GUI (the agent is the always-on process); logs and moves on if
/// the binary can't be found / started — the user may start it via systemd or
/// by hand, and the observe loop keeps retrying the connection regardless.
pub(super) fn spawn_agent() {
    if SUITE_QUITTING.load(std::sync::atomic::Ordering::Relaxed) {
        info!("suite is quitting — leaving the agent down");
        return;
    }
    let Some(path) = agent_binary_path() else {
        warn!(
            "agent not reachable and its binary wasn't found next to the GUI — \
             start it via systemd or by hand"
        );
        return;
    };
    match launch_agent(&path) {
        Ok(()) => info!(path = %path.display(), "agent not running — launch started"),
        Err(e) => warn!(error = %e, path = %path.display(), "could not launch the agent"),
    }
}

/// Launch the agent binary at `path` as a detached process.
fn launch_agent(path: &std::path::Path) -> std::io::Result<()> {
    disclaim::Command::new(path).spawn().map(|_| ())
}

/// Resolve the agent executable relative to the running GUI: its sibling in
/// the install prefix (`/usr/bin`) or the cargo target dir (dev).
fn agent_binary_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let sibling = dir.join(openlogi_core::brand::Helper::Agent.executable());
    sibling.exists().then_some(sibling)
}
