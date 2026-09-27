//! The agent's lifecycle as an explicit state machine.
//!
//! Every process start walks the same ladder, and each state is a type:
//!
//! ```text
//! startup::bootstrap ──► Booted ──gate──► Wanted ──arm──► Armed ──► Running ──► exit
//!         │                 │                                         │
//!         └─ init failed    └─ dormant start nobody wanted            └─ signal / process request
//! ```
//!
//! The moves are the type protection for these lifecycle contracts: the
//! shutdown-request receiver travels inside the states (gate consumes it first,
//! then the run loop — no third consumer can exist), the demand channel dies at
//! [`Wanted::arm`], and arming without settling the dormancy question is
//! unrepresentable — `arm` exists only on [`Wanted`], whose sole producer is the
//! gate. Moving `Armed` into `Running` also hands the single-consumer resume
//! stream to inventory exactly once. Linux only ever starts wanted (systemd
//! runs the unit only when it is enabled), so the gate passes unconditionally.

mod transition;

use std::sync::Arc;

use futures::StreamExt as _;
use openlogi_agent_core::event_monitor::EventMonitor;
use openlogi_agent_core::observable::ObservableState;
use openlogi_agent_core::orchestrator::{Orchestrator, SharedHandles};
use openlogi_agent_core::runtime::hook;
use openlogi_agent_core::watchers::foreground_app::ForegroundUpdate;
use openlogi_agent_core::watchers::inventory::{InventoryEvent, InventoryRefresh};
use openlogi_core::config::Config;
use openlogi_hook::Hook;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use self::transition::{Replacement, WatcherFleet};
use crate::shutdown::{self, ShutdownRequest, ShutdownRequests, ShutdownSignals};
use crate::startup::{self, Core, InputServices};
use crate::{autostart, overlay, server};

/// Walk the whole lifecycle: bootstrap, gate, arm, run. This is the async
/// core's entry point; `main` only decides which thread it runs on.
pub(crate) async fn run(config: Config, shutdown_requests: ShutdownRequests) {
    // Reconcile the agent's launch-at-login autostart before `config` moves
    // into the orchestrator.
    autostart::reconcile(config.app_settings.launch_at_login);

    let Some(booted) = Booted::bootstrap(config, shutdown_requests).await else {
        return;
    };
    let wanted = booted.gate();
    wanted.arm().run().await;
}

/// A bootstrapped, not-yet-armed agent: the IPC socket is serving, nothing
/// user-visible has happened. The only ways out are [`Self::gate`] and being
/// dropped (exit).
struct Booted {
    core: Core,
    signals: ShutdownSignals,
    /// The sole receiver for uninstall and replacement requests. It
    /// moves through the typestates with process-resource ownership.
    shutdown_requests: ShutdownRequests,
    /// The hook kill-switch, startup-only on purpose: flipping it requires
    /// an agent restart, which the config docs state.
    capture_mouse_events: bool,
}

impl Booted {
    async fn bootstrap(config: Config, shutdown_requests: ShutdownRequests) -> Option<Self> {
        // Read before `config` moves into the orchestrator.
        let capture_mouse_events = config.app_settings.capture_mouse_events;
        let core = startup::bootstrap(config).await?;
        Some(Self {
            core,
            signals: ShutdownSignals::install(),
            shutdown_requests,
            capture_mouse_events,
        })
    }

    /// Linux has no login trigger to second-guess: every start was asked
    /// for, so the gate passes unconditionally.
    fn gate(self) -> Wanted {
        Wanted(self)
    }
}

/// A booted agent whose dormancy question is settled: somebody wants it
/// running. [`Booted::gate`] is the only producer, so an agent that never
/// consulted the gate cannot arm.
struct Wanted(Booted);

impl Wanted {
    /// The arming point: the overlay may start,
    /// permissions may prompt, devices may open.
    fn arm(self) -> Armed {
        let Booted {
            core,
            signals,
            shutdown_requests,
            capture_mouse_events,
            ..
        } = self.0;
        overlay::spawn();
        prompt_missing_accessibility(capture_mouse_events);

        let Core {
            orchestrator,
            shared,
            observable,
            event_monitor,
            inputs,
            ring_haptics,
            demand,
        } = core;
        // Closing the channel turns post-arming declarations into no-ops in
        // the server's `declare_client` handler.
        drop(demand);
        Armed {
            running: Running {
                orchestrator,
                shared,
                observable,
                event_monitor,
                inputs,
                ring_haptics,
                signals,
                shutdown_requests,
                hidpp_watchers: WatcherFleet::Inactive,
                hook: None,
                capture_mouse_events,
            },
        }
    }
}

/// An armed agent ready to start its watcher fleets.
struct Armed {
    running: Running,
}

/// The live agent state into which the select loop folds events.
/// Separate from [`Armed`] so watcher startup and the steady-state event loop
/// remain distinct lifecycle phases.
struct Running {
    orchestrator: Arc<Mutex<Orchestrator>>,
    shared: SharedHandles,
    observable: Arc<ObservableState>,
    event_monitor: Arc<EventMonitor>,
    inputs: InputServices,
    ring_haptics: server::RingHapticPlayer,
    signals: ShutdownSignals,
    shutdown_requests: ShutdownRequests,
    hidpp_watchers: WatcherFleet,
    /// The OS hook, installed once input access is granted and dropped on
    /// revoke (dropping the handle stops its thread).
    hook: Option<Hook>,
    capture_mouse_events: bool,
}

impl Armed {
    /// Start the watcher fleets, then drain every control-plane source until
    /// told to leave (low-frequency by contract — [`startup::WatcherEvent`]).
    async fn run(self) {
        let Self { mut running } = self;

        // HID++ watchers need no Accessibility — start them up front.
        running.restart_hidpp_watchers();
        let (mut watchers, inventory_refresh) = startup::spawn_state_watchers(&running.shared);

        info!("openlogi-agent started");
        loop {
            tokio::select! {
                biased;

                () = running.signals.recv() => {
                    running.shut_down("shutdown signal").await;
                }
                Some(request) = running.shutdown_requests.recv() => {
                    running.handle_shutdown_request(request).await;
                }
                (request, stopped) = running.hidpp_watchers.replacement_ready() => {
                    running.complete_replacement(request, stopped);
                }
                Some(event) = watchers.next() => {
                    running.apply_watcher(event, &inventory_refresh).await;
                }
                Some(device_key) = running.inputs.triggers.recv() => {
                    running.begin_action_ring(device_key.as_deref()).await;
                }
                else => break,
            }
        }
    }
}

impl Running {
    /// Fold one watcher event into the agent's state.
    async fn apply_watcher(
        &mut self,
        event: startup::WatcherEvent,
        inventory_refresh: &InventoryRefresh,
    ) {
        use startup::{Watcher, WatcherEvent};

        match event {
            WatcherEvent::Inventory(event) => {
                self.apply_inventory(event, inventory_refresh).await;
            }
            WatcherEvent::Camera(active) => {
                self.orchestrator.lock().await.set_camera_active(active);
            }
            WatcherEvent::App(app) => self.apply_foreground(app).await,
            WatcherEvent::Pointer(context) => self.apply_pointer_context(context).await,
            WatcherEvent::Accessibility(granted) => self.apply_accessibility(granted).await,
            WatcherEvent::InputMonitoring(granted) => {
                self.observable.set_input_monitoring_granted(granted);
            }
            // Watcher thread death — without a snapshot the GUI would scan
            // forever.
            WatcherEvent::Lost(Watcher::Inventory) => {
                warn!("inventory watcher channel closed — marking enumeration unavailable");
                self.orchestrator.lock().await.mark_inventory_unavailable();
            }
            WatcherEvent::Lost(Watcher::Pointer) if openlogi_hook::pointer_context_supported() => {
                warn!("pointer watcher channel closed — disabling pointer-scoped remaps");
                self.apply_pointer_context(openlogi_hook::PointerContext {
                    app: None,
                    target: openlogi_hook::PointerTarget::Unavailable,
                })
                .await;
            }
            WatcherEvent::Lost(source) => debug!(?source, "state watcher channel closed"),
        }
    }

    /// Fold one inventory-watcher event into the orchestrator.
    async fn apply_inventory(&self, event: InventoryEvent, refresh: &InventoryRefresh) {
        match event {
            InventoryEvent::Snapshot {
                inventories,
                standalone,
                hid_open_failures,
            } => {
                let mut orchestrator = self.orchestrator.lock().await;
                orchestrator.refresh_inventory(&inventories, &standalone, hid_open_failures);
                let confirm_settings = orchestrator.needs_reapply_confirmation();
                drop(orchestrator);
                if confirm_settings {
                    refresh.request_settings_confirmation();
                }
            }
            InventoryEvent::Unavailable => {
                self.orchestrator.lock().await.mark_inventory_unavailable();
            }
            // Devices likely power-cycled during the sleep; the next snapshot
            // re-applies their volatile settings (#189).
            InventoryEvent::SystemWake => {
                self.orchestrator
                    .lock()
                    .await
                    .reapply_volatile_on_next_refresh();
            }
        }
    }

    /// Publish one foreground-app change and cancel button lifecycles whose
    /// bindings were resolved against the previous app profile.
    async fn apply_foreground(&self, app: ForegroundUpdate) {
        if self.orchestrator.lock().await.set_current_app(app) {
            self.inputs.dispatcher.cancel_all_buttons();
        }
    }

    async fn apply_pointer_context(&self, context: openlogi_hook::PointerContext) {
        let current = context.target;
        if self.orchestrator.lock().await.set_pointer_context(context) {
            self.inputs
                .dispatcher
                .cancel_pointer_buttons_except(current);
        }
    }

    async fn begin_action_ring(&self, device_key: Option<&str>) {
        // A second trigger press while the ring is showing closes it.
        if self.inputs.ring.dismiss_active() {
            return;
        }
        if let Some(session) = self
            .orchestrator
            .lock()
            .await
            .action_ring_session(device_key)
        {
            // Re-arm the firmware haptic engine first: power transitions can
            // clear it, after which plays are accepted without feedback.
            self.ring_haptics.arm(session.haptic_route.clone());
            self.inputs.ring.begin(session);
        }
    }

    /// Fold one Accessibility-grant change into the hook, then publish the
    /// permission and the hook state it produced as one generation — no
    /// observation can claim the hook is installed without the permission it
    /// requires.
    async fn apply_accessibility(&mut self, granted: bool) {
        if !granted {
            self.stop_hook();
        }
        if granted && self.hook.is_none() {
            self.hook = self.start_hook();
        }
        self.orchestrator
            .lock()
            .await
            .set_os_mouse_hook_available(self.hook.is_some());
        self.observable
            .set_accessibility_and_hook(granted, self.hook.is_some());
    }

    /// Install the OS mouse hook, or say why it stays off.
    fn start_hook(&self) -> Option<Hook> {
        if !self.capture_mouse_events {
            info!(
                "OS mouse hook disabled by app_settings.capture_mouse_events — \
                 button remapping is off"
            );
            return None;
        }
        info!("accessibility granted — installing OS mouse hook");
        hook::start(
            self.shared.hook_maps.clone(),
            self.shared.keyboard_bindings.clone(),
            self.inputs.dispatcher.clone(),
            self.inputs.scroll_input.clone(),
            Arc::clone(&self.event_monitor),
        )
    }

    /// Stop the hook so no new edge can race the lifecycle cancellation.
    fn stop_hook(&mut self) {
        self.hook = None;
        self.inputs.dispatcher.cancel_hook_buttons();
        self.inputs.scroll_input.cancel_hooks();
    }

    async fn handle_shutdown_request(&mut self, request: ShutdownRequest) {
        match request {
            // Uninstalled while running — leave through the same door so the
            // event tap and firmware diversions go with us (#807, #1097).
            ShutdownRequest::Uninstalled => {
                self.shut_down("the app was uninstalled").await;
            }
            ShutdownRequest::Restart { path, retry } => {
                self.hidpp_watchers
                    .begin_replacement(Replacement { path, retry });
            }
        }
    }

    /// Called only after the old fleet has acknowledged teardown. Failed
    /// teardown resumes the current image instead of replacing it.
    fn complete_replacement(&mut self, request: Replacement, stopped: bool) {
        if !stopped {
            warn!("HID++ teardown was unclean — refusing replacement and retrying");
            self.restart_hidpp_watchers();
            let _ = request.retry.send(());
            return;
        }
        self.restart(request);
    }

    fn restart_hidpp_watchers(&mut self) {
        self.hidpp_watchers =
            WatcherFleet::Running(startup::spawn_hidpp_watchers(&self.shared, &self.inputs));
    }

    fn restart(&mut self, Replacement { path, retry }: Replacement) {
        let error = crate::binary_watch::replace_process(&path);
        warn!(%error, path = %path.display(), "exec of the updated agent failed — restoring the current image and retrying");
        self.restart_hidpp_watchers();
        let _ = retry.send(());
    }

    async fn shut_down(&mut self, reason: &str) -> ! {
        std::mem::replace(&mut self.hidpp_watchers, WatcherFleet::Inactive)
            .stop_for_exit()
            .await;
        shutdown::release_hook_and_exit(self.hook.take(), &mut self.inputs, reason)
    }
}

/// Prompt for Accessibility when the enabled mouse hook needs it.
fn prompt_missing_accessibility(capture_mouse_events: bool) {
    // With the hook disabled the agent needs no Accessibility at all, so the
    // opt-out also silences that prompt.
    if capture_mouse_events && !Hook::has_accessibility() {
        Hook::prompt_accessibility();
    }
}
