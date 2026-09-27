//! How a restart is actually carried out: `exec` the new image in place, which
//! keeps the pid, the singleton lock, and the IPC socket.

use std::path::Path;

use tracing::info;

/// Restart this process as the new binary at `path`.
///
/// The lifecycle has already confirmed every HID++ manager stopped before
/// calling this. If `exec` fails, return the error so it can safely start a new
/// manager fleet before asking the binary watcher to retry.
pub(crate) fn replace_process(path: &Path) -> std::io::Error {
    use std::os::unix::process::CommandExt as _;
    info!(
        path = %path.display(),
        "executable changed on disk — restarting as the new binary"
    );
    // Forward our argv (none today) so a future flag survives the restart.
    std::process::Command::new(path)
        .args(std::env::args_os().skip(1))
        .exec()
}
