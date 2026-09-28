//! Omalogi background agent — headless, always-on.
//!
//! Owns the evdev hook and the HID++ device path (gesture capture, DPI,
//! SmartShift), serves the GUI over a Unix-socket tarpc IPC, and reconciles its
//! own systemd user-unit autostart. The async core walks the state machine in
//! `lifecycle` on a tokio runtime that owns the main thread.

mod autostart;
mod binary_watch;
mod lifecycle;
mod logging;
mod overlay;
mod pairing;
#[cfg(target_os = "linux")]
mod resume_linux;
// The shared locale catalogs live in `openlogi-ui`; the negotiation that picks
// one is `openlogi_core::locale`. `t!` resolves against a backend each binary
// generates itself, hence the relative path — see
// `tests::the_shared_catalog_is_wired_up` for why a wrong path is silent.
rust_i18n::i18n!("../openlogi-ui/locales", fallback = "en");
mod server;
mod shutdown;
mod startup;
mod takeover;

use openlogi_core::config::Config;
use tracing::{info, warn};

fn main() {
    logging::init();

    // Single-instance guard: the agent owns all device I/O, the input hook, and
    // the IPC socket, so a second agent must never start — systemd's restart
    // racing the GUI's one-shot auto-spawn could otherwise bring up two, and the
    // loser would steal the socket and install a duplicate event tap. Held for
    // the whole process; the OS releases it on exit (crash-recovery is free).
    let _guard = match openlogi_core::single_instance::acquire(
        openlogi_core::single_instance::Role::Agent,
    ) {
        Ok(g) => g,
        Err(openlogi_core::single_instance::InstanceError::AlreadyRunning { path }) => {
            // The holder may be a leftover from before this binary's update —
            // a pre-self-restart agent never exits on its own, and it would
            // wedge the (newer) GUI on its connecting screen forever. If it
            // provably speaks an older protocol, replace it; otherwise exit
            // as the duplicate we are.
            let Some(g) = takeover::try_replace_stale() else {
                info!(path = %path.display(), "another openlogi-agent is already running — exiting");
                return;
            };
            info!("replaced a stale agent — continuing as the new one");
            g
        }
        Err(e) => {
            warn!(error = %e, "single-instance check failed — exiting");
            return;
        }
    };

    // Every non-signal process transition reports to the lifecycle owner. In
    // particular, the binary watcher must not exec or exit from its own thread:
    // an armed lifecycle first releases firmware diversion.
    let (shutdown_tx, shutdown_requests) = shutdown::request_channel();
    // Only the lock-holding (real) agent watches, so a losing duplicate cannot
    // restart anything. The overlay spawns only after the lifecycle decides the
    // agent is wanted; a dormant agent must not bring a helper up.
    binary_watch::spawn(shutdown_tx.clone());

    let config = Config::load_or_default().unwrap_or_else(|e| {
        warn!(error = %e, "could not load config.toml; using defaults");
        Config::default()
    });
    // The tray renders localized strings; resolve the stored preference (or
    // the system locale) before any menu is built. A live language switch
    // reaches the running agent through `reload_config`.
    openlogi_core::locale::activate(config.app_settings.language.as_deref());
    // `[commands]` overrides; `reload_config` replaces them on every reload.
    openlogi_inject::set_command_overrides(config.commands.clone());

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            warn!(error = %e, "tokio runtime init failed; agent exiting");
            return;
        }
    };

    let device_io_signal = openlogi_hid::host::device_io_signal();
    #[cfg(target_os = "linux")]
    resume_linux::register(device_io_signal.clone());
    #[cfg(not(target_os = "linux"))]
    drop(device_io_signal);
    runtime.block_on(lifecycle::run(config, shutdown_requests));
}

#[cfg(test)]
mod tests;
