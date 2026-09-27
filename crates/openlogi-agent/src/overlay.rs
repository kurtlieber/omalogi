//! Supervision of the warm Actions Ring overlay helper.
//!
//! The helper owns no device state and exits harmlessly when its binary is not
//! packaged. Keeping it warm removes process-start latency from panel presses.
//!
//! Exactly one overlay may exist, and it belongs to one agent run. Both halves
//! are enforced by the `succession` crate: this supervisor waits while the role
//! is filled by its own child, and evicts a tenant left behind by a previous
//! agent — which is what stops an orphaned overlay from wedging its
//! replacement out of the lock forever (#621, #644). The run token travels to
//! the child in the environment, so a helper started by a previous agent is
//! recognizable on sight rather than after a timeout.

use std::path::{Path, PathBuf};
use std::process::Command;

use openlogi_core::brand;
use openlogi_ipc::RUN_ENV;
use succession::eviction::{self, AnonymousOutcome, Policy};
use succession::supervision::{Event, Supervisor};
use succession::{Role, Run};
use tracing::{info, warn};

/// Start the overlay supervisor on a dedicated thread.
pub fn spawn() {
    let Some(binary) = overlay_binary_path() else {
        warn!("Actions Ring overlay binary not found — overlay disabled");
        return;
    };
    let Ok(directory) = openlogi_core::paths::config_dir() else {
        warn!("could not resolve the config directory — overlay disabled");
        return;
    };
    let mine = Run::mint();
    let mut supervisor = Supervisor::new(Role::new(directory, "overlay"), mine);
    // The image an unidentified tenant is recognized by, kept beside the
    // spawn closure that owns the original.
    let helper = binary.clone();
    let result = std::thread::Builder::new()
        .name("openlogi-overlay-supervisor".into())
        .spawn(move || {
            let mut spawn = move || {
                Command::new(&binary)
                    .env(RUN_ENV, mine.get().to_string())
                    .spawn()
            };
            // The anonymous verdict repeats every poll for as long as the
            // tenant lives, and answering it walks the process table. Answer
            // once per spell of anonymity and stay quiet until the role
            // changes hands.
            let mut pressed_anonymous = false;
            loop {
                if let Err(error) = supervisor.tick(&mut spawn, &mut |event| {
                    report(&event, &helper, &mut pressed_anonymous);
                }) {
                    // A role that cannot be probed is treated as free by the
                    // next tick; refusing to look again would wait forever.
                    warn!(%error, "could not read the Actions Ring overlay role");
                }
            }
        });
    if let Err(error) = result {
        warn!(%error, "could not start the Actions Ring overlay supervisor");
    }
}

/// Log what the supervisor did, and evict a tenant this agent has superseded.
///
/// Eviction is the migration path: an overlay that predates the claim record
/// cannot recognize this agent as a different run, so it never yields on its
/// own. Which of the two evictions applies depends on what the tenant said
/// about itself, and neither takes a pid on faith — a record is checked
/// against the live process ([`succession::Tenant::compare`]), and a tenant
/// with no record at all is only ever recognized by the image we start the
/// overlay from.
fn report(event: &Event<'_>, helper: &Path, pressed_anonymous: &mut bool) {
    if !matches!(event, Event::SupersededAnonymously) {
        *pressed_anonymous = false;
    }
    match *event {
        Event::Superseded(record) => {
            info!("{event}");
            match eviction::evict(record, &Policy::default()) {
                eviction::Outcome::Refused(sameness) => {
                    warn!(
                        ?sameness,
                        "left the overlay alone — its pid no longer matches"
                    );
                }
                outcome => info!(?outcome, "asked the superseded overlay to leave"),
            }
        }
        // A tenant holding the role with no readable claim record: an overlay
        // from an install that predates the record, or one whose publish
        // failed. Nothing identifies it, so `succession` falls back on the
        // image we start the overlay from and refuses unless exactly one
        // process matches — otherwise the role stays wedged for as long as
        // that process lives and the Actions Ring never comes up (#842).
        Event::SupersededAnonymously => {
            if std::mem::replace(pressed_anonymous, true) {
                tracing::debug!("{event}");
                return;
            }
            warn!("{event}");
            match eviction::evict_anonymous(helper, &Policy::default()) {
                AnonymousOutcome::NoCandidate => warn!(
                    helper = %helper.display(),
                    "nothing is running our overlay binary — the role is held by something else"
                ),
                AnonymousOutcome::Ambiguous { running } => warn!(
                    running,
                    "several overlay processes are running — left them alone rather than \
                     guess which one holds the role"
                ),
                outcome => info!(?outcome, "asked the unidentified overlay to leave"),
            }
        }
        Event::Occupied(_) => tracing::debug!("{event}"),
        _ => info!("{event}"),
    }
}

fn overlay_binary_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let sibling = executable.parent()?.join(format!(
        "{}{}",
        brand::Helper::Overlay.executable(),
        std::env::consts::EXE_SUFFIX
    ));
    if sibling.is_file() {
        return Some(sibling);
    }

    find_on_path(brand::Helper::Overlay.executable())
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn path_search_returns_none_for_an_impossible_name() {
        assert_eq!(
            find_on_path("openlogi-overlay-this-file-does-not-exist"),
            None
        );
    }

    #[test]
    fn nested_overlay_path_has_expected_layout() {
        let outer = Path::new("/Applications/OpenLogi.app");
        assert_eq!(
            outer.join(
                "Contents/Library/LoginItems/OpenLogi Overlay.app/Contents/MacOS/openlogi-overlay"
            ),
            Path::new(
                "/Applications/OpenLogi.app/Contents/Library/LoginItems/OpenLogi Overlay.app/Contents/MacOS/openlogi-overlay"
            )
        );
    }
}
