//! Process-shutdown requests, `SIGTERM`/`SIGINT` listeners, and the one exit
//! path that releases firmware capture and the input hook before process end.

use std::path::PathBuf;

use openlogi_hook::Hook;
use tokio::sync::{mpsc, oneshot};
use tracing::info;
#[cfg(unix)]
use tracing::warn;

use crate::startup::InputServices;

/// A non-signal request for the process lifecycle owner.
pub(crate) enum ShutdownRequest {
    /// The installed application disappeared from disk.
    Uninstalled,
    /// The executable was replaced and the process should become `path`.
    Restart {
        path: PathBuf,
        /// Sent only when scheduling or `exec` failed and the binary watcher
        /// should resume observing for another settled replacement.
        retry: oneshot::Sender<()>,
    },
}

/// Sender cloned into the executable watcher. That thread requests
/// transitions; the lifecycle remains the sole authority that performs them.
pub(crate) type ShutdownRequestSender = mpsc::UnboundedSender<ShutdownRequest>;

/// The single non-signal shutdown source carried through every lifecycle
/// stage so process replacement cannot bypass armed firmware ownership.
pub(crate) struct ShutdownRequests(mpsc::UnboundedReceiver<ShutdownRequest>);

impl ShutdownRequests {
    pub(crate) async fn recv(&mut self) -> Option<ShutdownRequest> {
        self.0.recv().await
    }
}

/// Build the process-wide non-signal shutdown channel.
pub(crate) fn request_channel() -> (ShutdownRequestSender, ShutdownRequests) {
    let (tx, rx) = mpsc::unbounded_channel();
    (tx, ShutdownRequests(rx))
}

/// A future that fires when `signal` does, or never when the handler could not
/// be installed.
#[cfg(unix)]
async fn fires(signal: &mut Option<tokio::signal::unix::Signal>) {
    match signal {
        Some(signal) => {
            signal.recv().await;
        }
        None => std::future::pending::<()>().await,
    }
}

/// The stop-signal listeners, installed once and consumed by whichever
/// lifecycle stage is currently in charge.
pub(crate) struct ShutdownSignals {
    #[cfg(unix)]
    sigterm: Option<tokio::signal::unix::Signal>,
    #[cfg(unix)]
    sigint: Option<tokio::signal::unix::Signal>,
}

impl ShutdownSignals {
    /// Install the shutdown-signal handlers. A handler that cannot be
    /// installed is `None`, which simply never fires.
    #[cfg(unix)]
    pub(crate) fn install() -> Self {
        fn listen(kind: tokio::signal::unix::SignalKind) -> Option<tokio::signal::unix::Signal> {
            tokio::signal::unix::signal(kind)
                .inspect_err(|error| warn!(%error, ?kind, "could not install signal handler"))
                .ok()
        }
        Self {
            sigterm: listen(tokio::signal::unix::SignalKind::terminate()),
            sigint: listen(tokio::signal::unix::SignalKind::interrupt()),
        }
    }

    /// No signals exist off unix.
    #[cfg(not(unix))]
    pub(crate) fn install() -> Self {
        Self {}
    }

    /// Resolves on the first signal that means *stop now*: `SIGTERM` from
    /// systemd or a takeover, `SIGINT` from a dev-run Ctrl-C — both would
    /// otherwise kill the process with the event tap still armed.
    #[cfg(unix)]
    pub(crate) async fn recv(&mut self) {
        tokio::select! {
            () = fires(&mut self.sigterm) => {}
            () = fires(&mut self.sigint) => {}
        }
    }

    /// No signal to wait for off unix; the future simply never resolves.
    #[cfg(not(unix))]
    pub(crate) async fn recv(&mut self) {
        std::future::pending::<()>().await;
    }
}

/// Release the input hook, then end the process. The exit has to be explicit
/// so it runs the hook's destructor before the process ends.
pub(crate) fn release_hook_and_exit(
    hook: Option<Hook>,
    inputs: &mut InputServices,
    reason: &str,
) -> ! {
    info!(reason, "releasing the input hook and exiting");
    drop(hook);
    inputs.shutdown();
    #[expect(
        clippy::exit,
        reason = "a signalled shutdown must end the process, and the loop that observed it runs off the main thread"
    )]
    std::process::exit(0)
}
