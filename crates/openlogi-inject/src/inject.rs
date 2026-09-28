//! OS input-event synthesis for each [`Action`], split out of openlogi-core so
//! the core schema stays platform- and IO-free.
//!
//! [`execute`] is the single entry point: it dispatches to the Linux
//! synthesiser (`linux::execute`), which translates an [`Action`] into native
//! uinput events, D-Bus calls, or Hyprland/Omarchy helper invocations.

#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::sync::{LazyLock, Mutex, PoisonError, RwLock};

#[cfg(target_os = "linux")]
use openlogi_core::binding::KeyboardUsage;
use openlogi_core::binding::{Action, KeyCombo};
#[cfg(target_os = "linux")]
use openlogi_core::binding::{Script, WorkflowStep};
use openlogi_core::config::CommandOverrides;
use openlogi_core::scroll::ScrollDelta;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
use linux as platform;

/// Which isolated edge of a held keyboard chord to synthesize.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyPhase {
    Down,
    Up,
}

/// One physical keyboard output shared by held chords.
///
/// Cmd aliases Ctrl on Linux, so ownership is counted after that mapping is
/// resolved.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum HeldKey {
    Control,
    Shift,
    Alt,
    Key(KeyboardUsage),
}

#[cfg(target_os = "linux")]
#[derive(Debug, Default, PartialEq, Eq)]
struct HoldTransition {
    up: Vec<HeldKey>,
    down: Vec<HeldKey>,
}

/// Reference counts for physical keyboard outputs across active chords.
#[cfg(target_os = "linux")]
#[derive(Default)]
struct HeldOutput {
    owners: HashMap<HeldKey, usize>,
}

#[cfg(target_os = "linux")]
impl HeldOutput {
    fn transition(
        &mut self,
        released: Option<&KeyCombo>,
        pressed: Option<&KeyCombo>,
    ) -> HoldTransition {
        let released = released.map_or_else(Vec::new, held_keys);
        let pressed = pressed.map_or_else(Vec::new, held_keys);
        let before = self.owners.clone();

        for key in &released {
            match self.owners.get_mut(key) {
                Some(owners) if *owners > 1 => *owners -= 1,
                Some(_) => {
                    self.owners.remove(key);
                }
                None => {}
            }
        }
        for key in &pressed {
            *self.owners.entry(*key).or_default() += 1;
        }

        HoldTransition {
            up: released
                .into_iter()
                .filter(|key| before.contains_key(key) && !self.owners.contains_key(key))
                .collect(),
            down: pressed
                .into_iter()
                .filter(|key| !before.contains_key(key) && self.owners.contains_key(key))
                .collect(),
        }
    }
}

#[cfg(target_os = "linux")]
static HELD_OUTPUT: LazyLock<Mutex<HeldOutput>> =
    LazyLock::new(|| Mutex::new(HeldOutput::default()));

#[cfg(target_os = "linux")]
fn held_keys(combo: &KeyCombo) -> Vec<HeldKey> {
    let mut keys = Vec::with_capacity(4);
    if combo.has_command() || combo.has_control() {
        keys.push(HeldKey::Control);
    }
    if combo.has_shift() {
        keys.push(HeldKey::Shift);
    }
    if combo.has_option() {
        keys.push(HeldKey::Alt);
    }
    keys.push(HeldKey::Key(combo.key()));
    keys
}

/// A shortcut-table entry, parsed once into the chord it names. The tables are
/// hand-written constants, so a parse failure is a programming error.
#[cfg(target_os = "linux")]
fn parse_shortcut(text: &str) -> KeyCombo {
    text.parse()
        .unwrap_or_else(|error| unreachable!("hardcoded shortcut table entry {text:?}: {error}"))
}

/// Run a script off the caller's thread: a shell command, an AppleScript or a
/// workflow can take seconds, and the caller is the input hook.
#[cfg(target_os = "linux")]
fn dispatch_script(script: Script<'_>) {
    match script {
        Script::AppleScript(src) => {
            let src = src.to_string();
            std::thread::spawn(move || platform::run_apple_script(&src));
        }
        Script::ShellCommand(cmd) => {
            let cmd = cmd.to_string();
            std::thread::spawn(move || platform::run_shell_command(&cmd));
        }
        Script::Workflow(steps) => {
            let steps = steps.to_vec();
            std::thread::spawn(move || run_workflow(&steps));
        }
    }
}

/// Run workflow steps in order on the current (worker) thread, so a `Delay`
/// never stalls the event tap. Each step is one call into the platform
/// backend; a backend that cannot perform a step logs and moves on.
#[cfg(target_os = "linux")]
fn run_workflow(steps: &[WorkflowStep]) {
    for step in steps {
        match step {
            WorkflowStep::TypeText(text) => platform::type_text(text),
            WorkflowStep::PressKey(combo) => platform::press_combo(combo),
            WorkflowStep::Delay { millis } => {
                std::thread::sleep(std::time::Duration::from_millis(*millis));
            }
            WorkflowStep::RunAppleScript(src) => platform::run_apple_script(src),
            WorkflowStep::RunShellCommand(cmd) => platform::run_shell_command(cmd),
        }
    }
}

/// Synthesise the OS-level event for `action`.
///
/// Key and scroll events are injected via a lazily-created `uinput`
/// virtual device. Mouse clicks inject `BTN_*` events. Window-manager actions
/// route to Hyprland/Omarchy helpers on sessions exposing
/// `HYPRLAND_INSTANCE_SIGNATURE` (`hyprctl`, `omarchy-system-lock`,
/// `omarchy-capture-screenshot`, `omarchy-menu`) with legacy-chord fallback;
/// off Hyprland the GNOME/KDE chords apply and actions with no universal
/// Linux equivalent (`OmarchyMenu`, `FormerWorkspace`, `ToggleScratchpad`,
/// `AppsMenu`) are silently skipped (debug-logged). `CustomShortcut` maps
/// macOS `kVK_*` codes to Linux key codes; macOS Cmd maps to Ctrl.
///
/// Device-side actions (`CycleDpiPresets`, `SetDpiPreset`,
/// `ToggleSmartShift`) are handled at the hook/HID layer, logging a trace here.
///
/// On other platforms a warning is logged and the function returns
/// immediately — the binary compiles clean on all targets.
///
/// # Manual verification
///
/// `execute` is intentionally excluded from the automated test suite because
/// it would need to intercept the OS event queue. Smoke-test it manually:
/// bind a button to any action in the GUI and confirm the expected system event
/// fires when the button is pressed (or use the `inject_action` example).
pub fn execute(action: &Action) {
    #[cfg(target_os = "linux")]
    if let Some(command) = command_override(action) {
        let label = action.label();
        std::thread::spawn(move || platform::run_user_command(&label, &command));
        return;
    }
    if let Action::OpenApplication(target) = action {
        let expanded = shellexpand::tilde(target.path());
        if let Err(error) = opener::open(expanded.as_ref()) {
            tracing::warn!(
                %error,
                path = target.path(),
                "could not open configured application, folder, or URL"
            );
        }
        return;
    }

    cfg_select! {
        target_os = "linux" => {
            linux::execute(action);
        }
        _ => {
            tracing::warn!(
                action = action.label(),
                "execute unsupported on this platform"
            );
        }
    }
}

/// The `[commands]` overrides [`execute`] consults before any built-in
/// behaviour. The agent replaces them on startup and on every config reload.
#[cfg(target_os = "linux")]
static COMMAND_OVERRIDES: LazyLock<RwLock<CommandOverrides>> =
    LazyLock::new(|| RwLock::new(CommandOverrides::default()));

/// Replace the `[commands]` overrides (ADR-0005): from now on [`execute`] runs
/// the configured shell command instead of each overridden action.
pub fn set_command_overrides(overrides: CommandOverrides) {
    cfg_select! {
        target_os = "linux" => {
            if !overrides.is_empty() {
                tracing::info!(count = overrides.iter().count(), "[commands] overrides active");
            }
            *COMMAND_OVERRIDES.write().unwrap_or_else(PoisonError::into_inner) = overrides;
        }
        _ => {
            let _ = overrides;
        }
    }
}

#[cfg(target_os = "linux")]
fn command_override(action: &Action) -> Option<String> {
    COMMAND_OVERRIDES
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(action)
        .map(str::to_owned)
}

/// One synthetic held chord, released exactly once when dropped.
///
/// Keep this value with the physical press lifecycle. Replacing its chord
/// preserves physical keys shared by the old and new chords; cancellation,
/// shutdown, and unwinding all release the current chord through [`Drop`].
#[must_use = "dropping the held chord immediately releases its synthetic output"]
pub struct HeldChord {
    combo: KeyCombo,
}

impl HeldChord {
    /// Replace this held chord without releasing physical keys shared by both.
    pub fn replace(&mut self, combo: &KeyCombo) {
        let old = std::mem::replace(&mut self.combo, combo.clone());
        hold_transition(Some(&old), Some(&self.combo));
    }
}

impl Drop for HeldChord {
    fn drop(&mut self) {
        hold_transition(Some(&self.combo), None);
    }
}

/// Synthesise the down edge of `combo` and return its release owner.
///
/// Keep the returned [`HeldChord`] until the physical press ends. Prefer
/// [`execute`] when the caller does not own a matching terminal event.
pub fn press_hold(combo: &KeyCombo) -> HeldChord {
    // Construct the owner before posting the edge so unwinding from the
    // platform backend still balances any ownership transition it completed.
    let held = HeldChord {
        combo: combo.clone(),
    };
    hold_transition(None, Some(&held.combo));
    held
}

fn hold_transition(released: Option<&KeyCombo>, pressed: Option<&KeyCombo>) {
    cfg_select! {
        target_os = "linux" => {
            let mut output = HELD_OUTPUT.lock().unwrap_or_else(PoisonError::into_inner);
            let transition = output.transition(released, pressed);
            linux::hold_keys(&transition.up, KeyPhase::Up);
            linux::hold_keys(&transition.down, KeyPhase::Down);
        }
        _ => {
            tracing::warn!(
                "held shortcut output unsupported on this platform"
            );
        }
    }
}

/// Safari toolbar navigation via macOS Accessibility. Safari exists only on
/// macOS, so this always returns `false`; kept so upstream's shared agent
/// runtime compiles unchanged.
#[must_use]
pub fn ax_navigate_browser(pid: i32, forward: bool) -> bool {
    let _ = (pid, forward);
    false
}

/// Integer scroll units ready for a platform API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct QuantizedScroll {
    x: i32,
    y: i32,
}

/// Carries fractional platform units across frames so rounding never changes
/// the cumulative distance.
#[derive(Default)]
struct ScrollQuantizer {
    residual_x: f64,
    residual_y: f64,
}

impl ScrollQuantizer {
    fn quantize(&mut self, delta: ScrollDelta, units_per_input: f64) -> QuantizedScroll {
        QuantizedScroll {
            x: quantize_axis(&mut self.residual_x, delta.x(), units_per_input),
            y: quantize_axis(&mut self.residual_y, delta.y(), units_per_input),
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the rounded value is clamped to the i32 range before conversion"
)]
fn quantize_axis(residual: &mut f64, input: f64, units_per_input: f64) -> i32 {
    let exact = input.mul_add(units_per_input, *residual);
    let rounded = exact
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
    let output = rounded as i32;
    *residual = exact - f64::from(output);
    output
}

/// Synthesise a typed scroll distance at the current focus.
///
/// Fractional wheel ticks are retained until the platform can represent them,
/// so a sequence of high-resolution frames preserves its cumulative distance.
/// Non-finite input is rejected at this I/O boundary.
pub fn post_scroll(delta: ScrollDelta) {
    if !delta.is_finite() || (delta.x() == 0.0 && delta.y() == 0.0) {
        return;
    }
    cfg_select! {
        target_os = "linux" => {
            linux::post_scroll(delta);
        }
        _ => {
            let _ = delta;
        }
    }
}

/// Lifecycle phase of one synthetic smooth-scroll frame.
///
/// Linux has no equivalent wheel-event field: the phase is retained by the
/// runtime contract but only the frame's distance is injected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmoothScrollPhase {
    /// First output frame of a new animation.
    Began,
    /// An intermediate output frame, including frames after retargeting.
    Changed,
    /// Final frame, carrying any correction needed to reach the exact target.
    Ended,
    /// The capture source ended before the animation reached its target.
    Cancelled,
}

/// Synthesise one frame of a finite smooth-scroll animation.
///
/// Fractional wheel ticks are preserved through the native high-resolution
/// output. Non-finite distance is rejected at this I/O boundary.
pub fn post_smooth_scroll(delta: ScrollDelta, phase: SmoothScrollPhase) {
    if !delta.is_finite() {
        return;
    }
    let _ = phase;
    post_scroll(delta);
}

/// Return the `/dev/input/eventN` node for the action-injector uinput device,
/// initialising it if needed.
///
/// Intended for debugging and manual smoke-testing (e.g. attaching `evtest`
/// before firing [`execute`]). Returns `None` on non-Linux platforms or
/// when the device could not be created (e.g. `/dev/uinput` not writable).
#[cfg(target_os = "linux")]
#[must_use]
pub fn action_device_path() -> Option<std::path::PathBuf> {
    linux::device_node()
}

#[cfg(test)]
mod tests {
    use openlogi_core::scroll::ScrollDelta;

    #[cfg(target_os = "linux")]
    use openlogi_core::binding::KeyCombo;

    #[cfg(target_os = "linux")]
    use super::{HeldKey, HeldOutput, HoldTransition};
    use super::{QuantizedScroll, ScrollQuantizer};

    /// Synthetic high-resolution input: eight eighth-ticks must total exactly
    /// one Linux wheel detent (120 raw units). This is deterministic
    /// model data, not a hardware capture.
    #[test]
    fn fractional_frames_preserve_cumulative_wheel_distance() {
        let mut quantizer = ScrollQuantizer::default();
        let total = (0..8)
            .map(|_| {
                quantizer
                    .quantize(ScrollDelta::wheel_ticks(0.0, 0.125), 120.0)
                    .y
            })
            .sum::<i32>();
        assert_eq!(total, 120);
    }

    #[test]
    fn opposing_fractional_input_cancels_without_rounding_drift() {
        let mut quantizer = ScrollQuantizer::default();
        let forward = quantizer.quantize(ScrollDelta::wheel_ticks(0.25, 0.0), 120.0);
        let backward = quantizer.quantize(ScrollDelta::wheel_ticks(-0.25, 0.0), 120.0);
        assert_eq!(forward, QuantizedScroll { x: 30, y: 0 });
        assert_eq!(backward, QuantizedScroll { x: -30, y: 0 });
    }

    #[cfg(target_os = "linux")]
    fn combo(label: &str) -> KeyCombo {
        label.parse().expect("test shortcut must be valid")
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shared_control_stays_down_until_its_last_chord_ends() {
        let control_a = combo("Ctrl+A");
        let control_b = combo("Ctrl+B");
        let mut output = HeldOutput::default();

        assert_eq!(
            output.transition(None, Some(&control_a)),
            HoldTransition {
                up: vec![],
                down: vec![HeldKey::Control, HeldKey::Key(control_a.key())],
            }
        );
        assert_eq!(
            output.transition(None, Some(&control_b)),
            HoldTransition {
                up: vec![],
                down: vec![HeldKey::Key(control_b.key())],
            }
        );
        assert_eq!(
            output.transition(Some(&control_a), None),
            HoldTransition {
                up: vec![HeldKey::Key(control_a.key())],
                down: vec![],
            }
        );
        assert_eq!(
            output.transition(Some(&control_b), None),
            HoldTransition {
                up: vec![HeldKey::Control, HeldKey::Key(control_b.key())],
                down: vec![],
            }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn command_and_control_share_one_physical_output() {
        let command_a = combo("Cmd+A");
        let control_b = combo("Ctrl+B");
        let mut output = HeldOutput::default();

        output.transition(None, Some(&command_a));
        assert_eq!(
            output.transition(None, Some(&control_b)),
            HoldTransition {
                up: vec![],
                down: vec![HeldKey::Key(control_b.key())],
            }
        );
        assert_eq!(
            output.transition(Some(&command_a), None),
            HoldTransition {
                up: vec![HeldKey::Key(command_a.key())],
                down: vec![],
            }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn replacement_preserves_shared_physical_outputs() {
        let old = combo("Ctrl+A");
        let new = combo("Ctrl+B");
        let mut output = HeldOutput::default();

        output.transition(None, Some(&old));
        assert_eq!(
            output.transition(Some(&old), Some(&new)),
            HoldTransition {
                up: vec![HeldKey::Key(old.key())],
                down: vec![HeldKey::Key(new.key())],
            }
        );
    }
}
