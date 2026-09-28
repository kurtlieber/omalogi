//! Linux helpers for synthesising OS-level input events via a shared `uinput`
//! virtual device.
//!
//! The device is created lazily on first use. If `/dev/uinput` is inaccessible
//! (missing group membership or udev rule) every call logs a `warn` and returns
//! without panicking.

use std::io;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use zbus::blocking::Connection as DbusConn;

use openlogi_core::binding::{
    Action, Effect, KeyCombo, MediaKey, MouseButton, NativeAction, Shortcut,
};
use openlogi_core::scroll::ScrollDelta;

use super::{HeldKey, KeyPhase, QuantizedScroll, ScrollQuantizer};

const HIGH_RES_UNITS_PER_TICK: f64 = 120.0;

#[derive(Default)]
struct ScrollOutput {
    high_resolution: ScrollQuantizer,
    legacy: ScrollQuantizer,
}

static SCROLL_OUTPUT: LazyLock<Mutex<ScrollOutput>> =
    LazyLock::new(|| Mutex::new(ScrollOutput::default()));

/// Linux implementation: classify `action` into an [`Effect`] and inject the
/// resulting events via a shared `uinput` virtual device.
pub(super) fn execute(action: &Action) {
    match action.effect() {
        Effect::None => {}
        // Extra mouse buttons: BTN_SIDE/BTN_EXTRA are the evdev side
        // buttons ("back"/"forward") browsers handle natively.
        Effect::Click(button) => click(mouse_button_code(button)),
        Effect::Shortcut(shortcut) => press_combo(&combo(shortcut)),
        Effect::Key(combo) | Effect::HeldKey(combo) => press_combo(combo),
        Effect::Scroll { dx, dy } => dispatch_scroll(dx, dy),
        Effect::Media(key) => dispatch_media(key),
        Effect::Native(native) => dispatch_native(action, native),
        Effect::Script(script) => super::dispatch_script(script),
        Effect::Text(text) => type_text(text),
        Effect::AgentSide => {
            tracing::debug!(
                action = action.label(),
                "device action handled by hook/HID layer"
            );
        }
    }
}

fn mouse_button_code(button: MouseButton) -> KeyCode {
    match button {
        MouseButton::Left => KeyCode::BTN_LEFT,
        MouseButton::Right => KeyCode::BTN_RIGHT,
        MouseButton::Middle => KeyCode::BTN_MIDDLE,
        MouseButton::Back => KeyCode::BTN_SIDE,
        MouseButton::Forward => KeyCode::BTN_EXTRA,
    }
}

/// The Linux chord for each named [`Shortcut`].
///
/// Parsed through [`KeyCombo`]'s existing, tested `FromStr` rather than
/// hand-built keycode lists — the table stays a flat, auditable list of
/// chord strings instead of a second modifier-encoding call site.
fn combo(shortcut: Shortcut) -> KeyCombo {
    let text = match shortcut {
        Shortcut::Copy => "Ctrl+C",
        Shortcut::Paste => "Ctrl+V",
        Shortcut::Cut => "Ctrl+X",
        Shortcut::Undo => "Ctrl+Z",
        // Ctrl+Shift+Z matches the macOS ⌘⇧Z convention (see `Shortcut::Redo`
        // doc on `Action`); Ctrl+Y is the GTK/LibreOffice convention and is
        // left to a `CustomShortcut` binding.
        Shortcut::Redo => "Ctrl+Shift+Z",
        Shortcut::SelectAll => "Ctrl+A",
        Shortcut::Find => "Ctrl+F",
        Shortcut::Save => "Ctrl+S",
        Shortcut::BrowserBack => "Alt+Left",
        Shortcut::BrowserForward => "Alt+Right",
        Shortcut::NewTab => "Ctrl+T",
        Shortcut::CloseTab => "Ctrl+W",
        Shortcut::ReopenTab => "Ctrl+Shift+T",
        Shortcut::NextTab => "Ctrl+Tab",
        Shortcut::PrevTab => "Ctrl+Shift+Tab",
        Shortcut::ReloadPage => "Ctrl+R",
    };
    super::parse_shortcut(text)
}

/// Press an already-resolved chord: a table lookup from [`combo`] or a
/// user-recorded [`Action::CustomShortcut`]/`WorkflowStep::PressKey`.
pub(super) fn press_combo(combo: &KeyCombo) {
    let Some(key) = hid_usage_to_linux(combo.key().code()) else {
        tracing::warn!(
            usage = combo.key().code(),
            "shortcut usage has no Linux mapping — press ignored"
        );
        return;
    };
    press_key(&modifiers_to_keycodes(combo), key);
}

/// Emit one edge for the physical keys whose ownership changed.
pub(super) fn hold_keys(keys: &[HeldKey], phase: KeyPhase) {
    let keys: Vec<_> = keys.iter().filter_map(|key| held_keycode(*key)).collect();
    if !keys.is_empty() {
        emit(&held_key_events(&keys, phase));
    }
}

/// MPRIS targets the running media player; XF86 volume keys go to the
/// system mixer (PulseAudio/PipeWire) which is what users expect.
fn dispatch_media(key: MediaKey) {
    match key {
        MediaKey::PlayPause => mpris_command("PlayPause"),
        MediaKey::NextTrack => mpris_command("Next"),
        MediaKey::PrevTrack => mpris_command("Previous"),
        MediaKey::VolumeUp => press_key(&[], KeyCode::KEY_VOLUMEUP),
        MediaKey::VolumeDown => press_key(&[], KeyCode::KEY_VOLUMEDOWN),
        MediaKey::Mute => press_key(&[], KeyCode::KEY_MUTE),
    }
}

/// Dispatch a window-manager or power [`NativeAction`]. `action` is only
/// used for its label in debug logs.
///
/// Omalogi targets Omarchy/Hyprland: on sessions exposing
/// `HYPRLAND_INSTANCE_SIGNATURE` each action routes to `hyprctl` or an
/// `omarchy-*` helper with fixed argv (resolved via the agent's `PATH`)
/// and falls back to the legacy chord when the helper is missing or fails.
/// Off Hyprland the legacy GNOME/KDE chords apply unchanged.
fn dispatch_native(action: &Action, native: NativeAction) {
    // Sleep is compositor-independent (logind) — identical everywhere.
    if native == NativeAction::Sleep {
        sleep_system();
        return;
    }
    if on_hyprland() && dispatch_hyprland(action, native) {
        return;
    }
    let ctrl = KeyCode::KEY_LEFTCTRL;
    let alt = KeyCode::KEY_LEFTALT;
    match native {
        // Handled on Hyprland above; off Hyprland no universal Linux
        // equivalent exists and the compositor shortcut varies.
        NativeAction::OmarchyMenu
        | NativeAction::FormerWorkspace
        | NativeAction::ToggleScratchpad
        | NativeAction::AppsMenu => {
            tracing::debug!(
                action = action.label(),
                "no Linux equivalent — action skipped"
            );
        }
        // Ctrl+Alt+←/→ is the default in GNOME and KDE.
        NativeAction::PreviousWorkspace => press_key(&[ctrl, alt], KeyCode::KEY_LEFT),
        NativeAction::NextWorkspace => press_key(&[ctrl, alt], KeyCode::KEY_RIGHT),
        // logind LockSession() via the system bus; falls back to Super+L.
        NativeAction::LockScreen => lock_screen(),
        // Region vs full-screen capture depends on the desktop environment's
        // screenshot handler for Print Screen, so both map to the same key.
        NativeAction::Screenshot | NativeAction::CaptureRegion => {
            press_key(&[], KeyCode::KEY_SYSRQ);
        }
        NativeAction::Sleep => sleep_system(),
    }
}

/// A Hyprland/Omarchy session exposes `HYPRLAND_INSTANCE_SIGNATURE`.
fn on_hyprland() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

/// Hyprland/Omarchy dispatch for [`NativeAction`]: `true` when a helper ran
/// successfully, `false` to take the legacy chord path in [`dispatch_native`].
fn dispatch_hyprland(action: &Action, native: NativeAction) -> bool {
    let Some((program, args)) = hyprland_command(native) else {
        // Sleep has no Hyprland mapping by design — no failure.
        return false;
    };
    let handled = run_helper(program, args);
    if !handled {
        tracing::debug!(
            action = action.label(),
            "Hyprland helper failed — legacy fallback"
        );
    }
    handled
}

/// Fixed helper argv per [`NativeAction`]; `None` = no Hyprland mapping.
/// Pure table so tests pin it without spawning processes.
///
/// The Lua dispatchers mirror Omarchy's own bindings
/// (`default/hypr/bindings/tiling.lua`): `hyprctl dispatch` no longer accepts
/// the legacy dispatcher strings on Omarchy's Lua config layer.
fn hyprland_command(native: NativeAction) -> Option<(&'static str, &'static [&'static str])> {
    match native {
        // SUPER+SHIFT+TAB / SUPER+TAB.
        NativeAction::PreviousWorkspace => Some((
            "hyprctl",
            &["eval", "hl.dispatch(hl.dsp.focus({workspace='e-1'}))"],
        )),
        NativeAction::NextWorkspace => Some((
            "hyprctl",
            &["eval", "hl.dispatch(hl.dsp.focus({workspace='e+1'}))"],
        )),
        // NOTE: no logind attempt here — `LockSession` succeeding does not
        // mean hyprlock ran. `omarchy-system-lock` is the lock path.
        NativeAction::LockScreen => Some(("omarchy-system-lock", &[])),
        // The bare command is Omasnap's interactive smart picker (the PRINT
        // binding); a full-screen capture needs the explicit mode.
        NativeAction::Screenshot => Some(("omarchy-capture-screenshot", &["fullscreen"])),
        NativeAction::CaptureRegion => Some(("omarchy-capture-screenshot", &["region"])),
        // SUPER+CTRL+TAB.
        NativeAction::FormerWorkspace => Some((
            "hyprctl",
            &["eval", "hl.dispatch(hl.dsp.focus({workspace='previous'}))"],
        )),
        // SUPER+SPACE / SUPER+ALT+SPACE.
        NativeAction::OmarchyMenu => Some(("omarchy-menu", &["toggle"])),
        NativeAction::AppsMenu => Some(("omarchy-menu", &["toggle", "apps"])),
        // SUPER+S.
        NativeAction::ToggleScratchpad => Some((
            "hyprctl",
            &[
                "eval",
                "hl.dispatch(hl.dsp.workspace.toggle_special('scratchpad'))",
            ],
        )),
        // Sleep never reaches here (see `dispatch_native`).
        NativeAction::Sleep => None,
    }
}

/// How long the action worker waits for a helper to exit before treating it
/// as launched. Omasnap keeps running for its capture preview and the lock
/// helper for its screensaver teardown; neither may stall later remaps.
const HELPER_EXIT_WAIT: Duration = Duration::from_secs(1);

/// Run one helper with fixed argv (no shell) and report success.
///
/// A helper that exits within [`HELPER_EXIT_WAIT`] reports its exit status; one
/// still running after that is treated as launched and reaped on a detached
/// thread. Output goes to `/dev/null`, so a backgrounded grandchild holding the
/// helper's stdout can't keep the worker waiting either.
fn run_helper(program: &str, args: &[&str]) -> bool {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            tracing::debug!(program, error = %e, "Hyprland helper not found");
            return false;
        }
    };
    let deadline = Instant::now() + HELPER_EXIT_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                tracing::debug!(program, "Hyprland helper ran");
                return true;
            }
            Ok(Some(status)) => {
                tracing::debug!(program, ?status, "Hyprland helper failed");
                return false;
            }
            Ok(None) if Instant::now() >= deadline => {
                tracing::debug!(
                    program,
                    "Hyprland helper still running — treated as launched"
                );
                std::thread::spawn(move || child.wait());
                return true;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                tracing::debug!(program, error = %e, "Hyprland helper wait failed");
                return false;
            }
        }
    }
}

/// Synthesise one scroll tick in direction `(dx, dy)`. Unit direction
/// (-1/0/1) scaled by the fixed relative-axis magnitude the four
/// `Scroll*`/`HorizontalScroll*` actions have always used.
fn dispatch_scroll(dx: i8, dy: i8) {
    if dy != 0 {
        scroll(RelativeAxisCode::REL_WHEEL, i32::from(dy) * 3);
    }
    if dx != 0 {
        scroll(RelativeAxisCode::REL_HWHEEL, i32::from(dx) * 3);
    }
}

/// Not implemented yet: unicode text has no uinput encoding without a keymap.
pub(super) fn type_text(text: &str) {
    tracing::warn!(
        chars = text.chars().count(),
        "TypeText injection is not implemented on Linux yet"
    );
}

pub(super) fn run_apple_script(_src: &str) {
    tracing::warn!("RunAppleScript is only supported on macOS");
}

pub(super) fn run_shell_command(cmd: &str) {
    run_user_command("Run Command", cmd);
}

/// Run a user-written shell string to completion on the calling thread —
/// always a thread of its own, never the action worker — and report failure.
///
/// Stdio is `/dev/null`, so a backgrounded grandchild that inherits stdout
/// cannot keep this thread waiting after the command itself exits. A spawn
/// error or a non-zero exit is logged and raised as a desktop notification
/// (rate-limited per action); nothing falls back to the built-in action,
/// because a user's command replaces it (ADR-0005).
pub(super) fn run_user_command(label: &str, cmd: &str) {
    let status = Command::new("/bin/sh")
        .args(["-c", cmd])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let failure = match status {
        Ok(status) if status.success() => {
            tracing::debug!(action = label, "user command ran");
            return;
        }
        Ok(status) => status.code().map_or_else(
            || format!("`{cmd}` was killed ({status})"),
            |code| format!("`{cmd}` exited with status {code}"),
        ),
        Err(error) => format!("`{cmd}` could not start: {error}"),
    };
    tracing::warn!(action = label, %failure, "user command failed");
    if COMMAND_FAILURES.should_notify(label, Instant::now()) {
        let _ = Command::new("notify-send")
            .args([
                "--app-name=Omalogi",
                &format!("{label}: command failed"),
                &failure,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// At most one failure notification per action in this window, so a button
/// mashed against a broken script produces one popup, not a stack of them.
const FAILURE_NOTIFY_INTERVAL: Duration = Duration::from_secs(10);

static COMMAND_FAILURES: FailureThrottle = FailureThrottle::new();

/// Remembers when each action last raised a failure notification.
struct FailureThrottle {
    last: Mutex<Vec<(String, Instant)>>,
}

impl FailureThrottle {
    const fn new() -> Self {
        Self {
            last: Mutex::new(Vec::new()),
        }
    }

    fn should_notify(&self, label: &str, now: Instant) -> bool {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        match last.iter_mut().find(|(seen, _)| seen == label) {
            Some((_, at)) if now.duration_since(*at) < FAILURE_NOTIFY_INTERVAL => false,
            Some((_, at)) => {
                *at = now;
                true
            }
            None => {
                last.push((label.to_owned(), now));
                true
            }
        }
    }
}

/// Must keep the `OpenLogi ` prefix: the hook refuses to grab any device whose
/// name starts with it (`openlogi-hook`'s `OPENLOGI_DEVICE_PREFIX`), and the
/// name is an internal identifier, not branding (ADR-0001).
const DEVICE_NAME: &str = "OpenLogi action injector";

static VIRTUAL_INPUT: LazyLock<Option<Mutex<VirtualDevice>>> = LazyLock::new(|| {
    build()
        .map(Mutex::new)
        .map_err(|e| tracing::warn!("failed to create uinput action device: {e}"))
        .ok()
});

#[rustfmt::skip]
const KEY_CAPABILITIES: &[KeyCode] = &[
    // Letters
    KeyCode::KEY_A, KeyCode::KEY_B, KeyCode::KEY_C, KeyCode::KEY_D,
    KeyCode::KEY_E, KeyCode::KEY_F, KeyCode::KEY_G, KeyCode::KEY_H,
    KeyCode::KEY_I, KeyCode::KEY_J, KeyCode::KEY_K, KeyCode::KEY_L,
    KeyCode::KEY_M, KeyCode::KEY_N, KeyCode::KEY_O, KeyCode::KEY_P,
    KeyCode::KEY_Q, KeyCode::KEY_R, KeyCode::KEY_S, KeyCode::KEY_T,
    KeyCode::KEY_U, KeyCode::KEY_V, KeyCode::KEY_W, KeyCode::KEY_X,
    KeyCode::KEY_Y, KeyCode::KEY_Z,
    // Digits
    KeyCode::KEY_0, KeyCode::KEY_1, KeyCode::KEY_2, KeyCode::KEY_3,
    KeyCode::KEY_4, KeyCode::KEY_5, KeyCode::KEY_6, KeyCode::KEY_7,
    KeyCode::KEY_8, KeyCode::KEY_9,
    // Punctuation / symbols
    KeyCode::KEY_MINUS,      KeyCode::KEY_EQUAL,   KeyCode::KEY_LEFTBRACE,
    KeyCode::KEY_RIGHTBRACE, KeyCode::KEY_BACKSLASH, KeyCode::KEY_SEMICOLON,
    KeyCode::KEY_APOSTROPHE, KeyCode::KEY_GRAVE,   KeyCode::KEY_COMMA,
    KeyCode::KEY_DOT,        KeyCode::KEY_SLASH,
    // Navigation / editing
    KeyCode::KEY_LEFT,  KeyCode::KEY_RIGHT, KeyCode::KEY_UP,       KeyCode::KEY_DOWN,
    KeyCode::KEY_HOME,  KeyCode::KEY_END,   KeyCode::KEY_PAGEUP,   KeyCode::KEY_PAGEDOWN,
    KeyCode::KEY_TAB,   KeyCode::KEY_ENTER, KeyCode::KEY_BACKSPACE, KeyCode::KEY_DELETE,
    KeyCode::KEY_ESC,   KeyCode::KEY_SPACE,
    // Modifiers (KEY_LEFTMETA used by the LockScreen Super+L fallback)
    KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_LEFTALT, KeyCode::KEY_LEFTMETA,
    // Function keys
    KeyCode::KEY_F1,  KeyCode::KEY_F2,  KeyCode::KEY_F3,  KeyCode::KEY_F4,
    KeyCode::KEY_F5,  KeyCode::KEY_F6,  KeyCode::KEY_F7,  KeyCode::KEY_F8,
    KeyCode::KEY_F9,  KeyCode::KEY_F10, KeyCode::KEY_F11, KeyCode::KEY_F12,
    KeyCode::KEY_F13, KeyCode::KEY_F14, KeyCode::KEY_F15, KeyCode::KEY_F16,
    KeyCode::KEY_F17, KeyCode::KEY_F18, KeyCode::KEY_F19, KeyCode::KEY_F20,
    // System
    KeyCode::KEY_SYSRQ,
    // Multimedia
    KeyCode::KEY_PLAYPAUSE, KeyCode::KEY_NEXTSONG, KeyCode::KEY_PREVIOUSSONG,
    KeyCode::KEY_VOLUMEUP,  KeyCode::KEY_VOLUMEDOWN, KeyCode::KEY_MUTE,
    // Mouse buttons (injected as EV_KEY with BTN_* codes). The side pair
    // must be registered here or the kernel silently drops their events.
    KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE,
    KeyCode::BTN_SIDE, KeyCode::BTN_EXTRA,
];

fn build() -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::default();
    for &k in KEY_CAPABILITIES {
        keys.insert(k);
    }

    // Only scroll axes: the device never emits cursor movement, so leaving
    // out REL_X/REL_Y keeps libinput from classifying it as a pointer —
    // which can otherwise cause injected key/wheel events to be grabbed by
    // pointer-grabbing X11 clients or routed oddly by some Wayland compositors.
    let mut axes = AttributeSet::<RelativeAxisCode>::default();
    for a in [
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL_HI_RES,
        RelativeAxisCode::REL_HWHEEL_HI_RES,
    ] {
        axes.insert(a);
    }

    VirtualDevice::builder()?
        .name(DEVICE_NAME)
        .with_keys(&keys)?
        .with_relative_axes(&axes)?
        .build()
}

fn emit(events: &[InputEvent]) {
    if let Some(m) = &*VIRTUAL_INPUT {
        if let Ok(mut guard) = m.lock() {
            if let Err(e) = guard.emit(events) {
                tracing::warn!("uinput action emit failed: {e}");
            }
        } else {
            tracing::warn!("uinput action device mutex poisoned");
        }
    } else {
        // Device creation failed at init; already logged once in LazyLock.
        tracing::debug!("uinput action device unavailable — action skipped");
    }
}

fn syn() -> InputEvent {
    InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0)
}

fn key_ev(code: KeyCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::KEY.0, code.0, value)
}

fn rel_ev(axis: RelativeAxisCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::RELATIVE.0, axis.0, value)
}

/// Inject modifier-down + key-down in one SYN frame, then key-up +
/// modifier-up in a second SYN frame.
///
/// Two separate frames give the kernel distinct timestamps for press and
/// release, which matches what the kernel `uinput` docs show and avoids
/// toolkits treating a zero-duration event as invalid.
fn press_key(mods: &[KeyCode], key: KeyCode) {
    emit(&key_phase_events(mods, key, KeyPhase::Down));
    emit(&key_phase_events(mods, key, KeyPhase::Up));
}

/// Build one `SYN_REPORT` frame. Down order is modifiers then key; up order
/// is the exact reverse so the ordinary key never escapes as an unmodified
/// release.
fn key_phase_events(mods: &[KeyCode], key: KeyCode, phase: KeyPhase) -> Vec<InputEvent> {
    let mut keys = Vec::with_capacity(mods.len() + 1);
    keys.extend_from_slice(mods);
    keys.push(key);
    held_key_events(&keys, phase)
}

fn held_key_events(keys: &[KeyCode], phase: KeyPhase) -> Vec<InputEvent> {
    let mut events = Vec::with_capacity(keys.len() + 1);
    match phase {
        KeyPhase::Down => {
            events.extend(keys.iter().map(|key| key_ev(*key, 1)));
        }
        KeyPhase::Up => {
            events.extend(keys.iter().rev().map(|key| key_ev(*key, 0)));
        }
    }
    events.push(syn());
    events
}

/// Inject a button-down in one SYN frame and button-up in a second.
fn click(button: KeyCode) {
    emit(&[key_ev(button, 1), syn()]);
    emit(&[key_ev(button, 0), syn()]);
}

/// Inject a single relative-axis delta followed by `SYN_REPORT`.
fn scroll(axis: RelativeAxisCode, value: i32) {
    emit(&[rel_ev(axis, value), syn()]);
}

pub(super) fn post_scroll(delta: ScrollDelta) {
    let ScrollDelta::WheelTicks { .. } = delta else {
        tracing::debug!("pixel scroll output is unsupported on Linux");
        return;
    };
    let Ok(mut output) = SCROLL_OUTPUT.lock() else {
        tracing::warn!("Linux scroll quantizer mutex poisoned");
        return;
    };
    let high_resolution = output
        .high_resolution
        .quantize(delta, HIGH_RES_UNITS_PER_TICK);
    let legacy = output.legacy.quantize(delta, 1.0);
    drop(output);

    let mut events = Vec::with_capacity(5);
    push_scroll_axes(
        &mut events,
        high_resolution,
        RelativeAxisCode::REL_HWHEEL_HI_RES,
        RelativeAxisCode::REL_WHEEL_HI_RES,
    );
    push_scroll_axes(
        &mut events,
        legacy,
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL,
    );
    if !events.is_empty() {
        events.push(syn());
        emit(&events);
    }
}

fn push_scroll_axes(
    events: &mut Vec<InputEvent>,
    delta: QuantizedScroll,
    horizontal: RelativeAxisCode,
    vertical: RelativeAxisCode,
) {
    if delta.x != 0 {
        events.push(rel_ev(horizontal, delta.x));
    }
    if delta.y != 0 {
        events.push(rel_ev(vertical, delta.y));
    }
}

/// Force the virtual device to initialise (if it hasn't already) and return
/// its `/dev/input/eventN` node path.
///
/// Uses `VirtualDevice::enumerate_dev_nodes()` which returns the correct
/// `/dev/input/eventN` path directly. Returns `None` if the device couldn't
/// be created or if the node hasn't appeared yet (udev typically creates it
/// within a few milliseconds of the `ioctl`).
pub(super) fn device_node() -> Option<std::path::PathBuf> {
    // Touch the LazyLock to force initialisation.
    let _ = &*VIRTUAL_INPUT;
    // Give udev a moment to create the /dev node.
    std::thread::sleep(std::time::Duration::from_millis(150));
    if let Some(m) = &*VIRTUAL_INPUT
        && let Ok(mut guard) = m.lock()
    {
        return guard.enumerate_dev_nodes_blocking().ok()?.flatten().next();
    }
    None
}

/// Convert a [`KeyCombo`] modifier bitmask
/// to the evdev keys to hold.
///
/// macOS Cmd (`MOD_CMD`) and Ctrl (`MOD_CTRL`) both map to `KEY_LEFTCTRL`;
/// the bitwise-OR check deduplicates them so at most one Ctrl is pushed.
/// Order is canonical: Ctrl → Shift → Alt.
fn modifiers_to_keycodes(combo: &openlogi_core::binding::KeyCombo) -> Vec<KeyCode> {
    let mut modifiers = Vec::new();
    if combo.has_command() || combo.has_control() {
        modifiers.push(KeyCode::KEY_LEFTCTRL);
    }
    if combo.has_shift() {
        modifiers.push(KeyCode::KEY_LEFTSHIFT);
    }
    if combo.has_option() {
        modifiers.push(KeyCode::KEY_LEFTALT);
    }
    modifiers
}

fn held_keycode(key: HeldKey) -> Option<KeyCode> {
    match key {
        HeldKey::Control => Some(KeyCode::KEY_LEFTCTRL),
        HeldKey::Shift => Some(KeyCode::KEY_LEFTSHIFT),
        HeldKey::Alt => Some(KeyCode::KEY_LEFTALT),
        HeldKey::Key(usage) => {
            let key = hid_usage_to_linux(usage.code());
            if key.is_none() {
                tracing::warn!(
                    usage = usage.code(),
                    "held shortcut usage has no Linux mapping — edge ignored"
                );
            }
            key
        }
    }
}

/// Map a platform-neutral USB HID keyboard usage to evdev.
fn hid_usage_to_linux(usage: u8) -> Option<KeyCode> {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::KEY_A,
        KeyCode::KEY_B,
        KeyCode::KEY_C,
        KeyCode::KEY_D,
        KeyCode::KEY_E,
        KeyCode::KEY_F,
        KeyCode::KEY_G,
        KeyCode::KEY_H,
        KeyCode::KEY_I,
        KeyCode::KEY_J,
        KeyCode::KEY_K,
        KeyCode::KEY_L,
        KeyCode::KEY_M,
        KeyCode::KEY_N,
        KeyCode::KEY_O,
        KeyCode::KEY_P,
        KeyCode::KEY_Q,
        KeyCode::KEY_R,
        KeyCode::KEY_S,
        KeyCode::KEY_T,
        KeyCode::KEY_U,
        KeyCode::KEY_V,
        KeyCode::KEY_W,
        KeyCode::KEY_X,
        KeyCode::KEY_Y,
        KeyCode::KEY_Z,
    ];
    const DIGITS: [KeyCode; 10] = [
        KeyCode::KEY_1,
        KeyCode::KEY_2,
        KeyCode::KEY_3,
        KeyCode::KEY_4,
        KeyCode::KEY_5,
        KeyCode::KEY_6,
        KeyCode::KEY_7,
        KeyCode::KEY_8,
        KeyCode::KEY_9,
        KeyCode::KEY_0,
    ];
    const FUNCTIONS: [KeyCode; 20] = [
        KeyCode::KEY_F1,
        KeyCode::KEY_F2,
        KeyCode::KEY_F3,
        KeyCode::KEY_F4,
        KeyCode::KEY_F5,
        KeyCode::KEY_F6,
        KeyCode::KEY_F7,
        KeyCode::KEY_F8,
        KeyCode::KEY_F9,
        KeyCode::KEY_F10,
        KeyCode::KEY_F11,
        KeyCode::KEY_F12,
        KeyCode::KEY_F13,
        KeyCode::KEY_F14,
        KeyCode::KEY_F15,
        KeyCode::KEY_F16,
        KeyCode::KEY_F17,
        KeyCode::KEY_F18,
        KeyCode::KEY_F19,
        KeyCode::KEY_F20,
    ];
    match usage {
        0x04..=0x1d => LETTERS.get(usize::from(usage - 0x04)).copied(),
        0x1e..=0x27 => DIGITS.get(usize::from(usage - 0x1e)).copied(),
        0x3a..=0x45 => FUNCTIONS.get(usize::from(usage - 0x3a)).copied(),
        0x68..=0x6f => FUNCTIONS.get(usize::from(usage - 0x68 + 12)).copied(),
        0x28 => Some(KeyCode::KEY_ENTER),
        0x29 => Some(KeyCode::KEY_ESC),
        0x2a => Some(KeyCode::KEY_BACKSPACE),
        0x2b => Some(KeyCode::KEY_TAB),
        0x2c => Some(KeyCode::KEY_SPACE),
        0x2d => Some(KeyCode::KEY_MINUS),
        0x2e => Some(KeyCode::KEY_EQUAL),
        0x2f => Some(KeyCode::KEY_LEFTBRACE),
        0x30 => Some(KeyCode::KEY_RIGHTBRACE),
        0x31 => Some(KeyCode::KEY_BACKSLASH),
        0x33 => Some(KeyCode::KEY_SEMICOLON),
        0x34 => Some(KeyCode::KEY_APOSTROPHE),
        0x35 => Some(KeyCode::KEY_GRAVE),
        0x36 => Some(KeyCode::KEY_COMMA),
        0x37 => Some(KeyCode::KEY_DOT),
        0x38 => Some(KeyCode::KEY_SLASH),
        0x4a => Some(KeyCode::KEY_HOME),
        0x4b => Some(KeyCode::KEY_PAGEUP),
        0x4c => Some(KeyCode::KEY_DELETE),
        0x4d => Some(KeyCode::KEY_END),
        0x4e => Some(KeyCode::KEY_PAGEDOWN),
        0x4f => Some(KeyCode::KEY_RIGHT),
        0x50 => Some(KeyCode::KEY_LEFT),
        0x51 => Some(KeyCode::KEY_DOWN),
        0x52 => Some(KeyCode::KEY_UP),
        _ => None,
    }
}

// ── D-Bus helpers ────────────────────────────────────────────────────────

static SESSION_BUS: LazyLock<Option<DbusConn>> = LazyLock::new(|| {
    DbusConn::session()
        .map_err(|e| tracing::warn!("D-Bus session bus unavailable: {e}"))
        .ok()
});

static SYSTEM_BUS: LazyLock<Option<DbusConn>> = LazyLock::new(|| {
    DbusConn::system()
        .map_err(|e| tracing::warn!("D-Bus system bus unavailable: {e}"))
        .ok()
});

/// Lock the screen via logind `LockSession($XDG_SESSION_ID)` on the system
/// bus, falling back to Super+L.
///
/// Only the session identified by `$XDG_SESSION_ID` is locked; if the
/// variable is unset the D-Bus path is skipped entirely to avoid locking
/// all sessions on the machine. Super+L covers non-systemd systems and the
/// no-session-id case.
fn lock_screen() {
    if let (Some(conn), Ok(id)) = (SYSTEM_BUS.as_ref(), std::env::var("XDG_SESSION_ID")) {
        match conn.call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            Some("org.freedesktop.login1.Manager"),
            "LockSession",
            &(id.as_str(),),
        ) {
            Ok(_) => {
                tracing::debug!("LockScreen via logind");
                return;
            }
            Err(e) => tracing::warn!("logind LockSession failed: {e}"),
        }
    }
    // Super+L is the standard lock shortcut on GNOME and KDE.
    tracing::debug!("LockScreen via Super+L key combo");
    press_key(&[KeyCode::KEY_LEFTMETA], KeyCode::KEY_L);
}

/// Suspend the system via logind's `Suspend()` on the system bus. The
/// `false` argument declines the "interactive" polkit prompt — if the
/// session isn't allowed to suspend, the call fails and is logged rather
/// than popping an authentication dialog from a background agent.
fn sleep_system() {
    let Some(conn) = SYSTEM_BUS.as_ref() else {
        tracing::warn!("no system bus — Sleep skipped");
        return;
    };
    match conn.call_method(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1",
        Some("org.freedesktop.login1.Manager"),
        "Suspend",
        &(false,),
    ) {
        Ok(_) => tracing::debug!("Sleep via logind Suspend"),
        Err(e) => tracing::warn!("logind Suspend failed: {e}"),
    }
}

/// Send `command` to the first MPRIS-capable media player on the session bus,
/// falling back to the corresponding XF86 multimedia key only if no MPRIS
/// player is found. When a player is found but the call fails, the fallback
/// is suppressed to avoid double-toggling (the player likely handles the
/// XF86 key too).
fn mpris_command(command: &str) {
    if try_mpris_command(command).is_none() {
        let fallback = match command {
            "PlayPause" => KeyCode::KEY_PLAYPAUSE,
            "Next" => KeyCode::KEY_NEXTSONG,
            "Previous" => KeyCode::KEY_PREVIOUSSONG,
            _ => return,
        };
        press_key(&[], fallback);
    }
}

fn try_mpris_command(command: &str) -> Option<()> {
    let conn = SESSION_BUS.as_ref()?;
    let reply = conn
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListNames",
            &(),
        )
        .ok()?;
    let names = reply.body().deserialize::<Vec<String>>().ok()?;
    let Some(player) = names
        .iter()
        .find(|n| n.starts_with("org.mpris.MediaPlayer2."))
    else {
        tracing::debug!("no MPRIS player found — {command} via XF86 key fallback");
        return None;
    };
    match conn.call_method(
        Some(player.as_str()),
        "/org/mpris/MediaPlayer2",
        Some("org.mpris.MediaPlayer2.Player"),
        command,
        &(),
    ) {
        Ok(_) => {
            tracing::debug!("MPRIS {command} via {player}");
            Some(())
        }
        Err(e) => {
            // Player was identified — suppress XF86 fallback to avoid
            // double-toggling if the player also handles multimedia keys.
            tracing::warn!("MPRIS {command} on {player} failed: {e}");
            Some(())
        }
    }
}

#[cfg(test)]
mod tests {
    use evdev::KeyCode;
    use openlogi_core::binding::{KeyCombo, NativeAction, Shortcut};

    use std::time::{Duration, Instant};

    use super::{
        FAILURE_NOTIFY_INTERVAL, FailureThrottle, combo, hid_usage_to_linux, hyprland_command,
        key_ev, key_phase_events, modifiers_to_keycodes, run_helper, run_user_command, syn,
    };
    use crate::inject::KeyPhase;

    #[test]
    fn held_chord_edges_use_inverse_key_order() {
        let modifiers = [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTSHIFT];
        assert_eq!(
            key_phase_events(&modifiers, KeyCode::KEY_P, KeyPhase::Down),
            vec![
                key_ev(KeyCode::KEY_LEFTCTRL, 1),
                key_ev(KeyCode::KEY_LEFTSHIFT, 1),
                key_ev(KeyCode::KEY_P, 1),
                syn(),
            ]
        );
        assert_eq!(
            key_phase_events(&modifiers, KeyCode::KEY_P, KeyPhase::Up),
            vec![
                key_ev(KeyCode::KEY_P, 0),
                key_ev(KeyCode::KEY_LEFTSHIFT, 0),
                key_ev(KeyCode::KEY_LEFTCTRL, 0),
                syn(),
            ]
        );
    }

    #[test]
    fn modifiers_map_to_linux_without_duplicate_control() {
        let combo = "Cmd+Ctrl+Shift+Alt+A"
            .parse::<KeyCombo>()
            .expect("a valid shortcut must parse");
        assert_eq!(
            modifiers_to_keycodes(&combo),
            vec![
                KeyCode::KEY_LEFTCTRL,
                KeyCode::KEY_LEFTSHIFT,
                KeyCode::KEY_LEFTALT
            ]
        );
    }

    #[test]
    fn hid_usages_map_letters_navigation_and_function_keys() {
        assert_eq!(hid_usage_to_linux(0x04), Some(KeyCode::KEY_A));
        assert_eq!(hid_usage_to_linux(0x50), Some(KeyCode::KEY_LEFT));
        assert_eq!(hid_usage_to_linux(0x3a), Some(KeyCode::KEY_F1));
        assert_eq!(hid_usage_to_linux(0x6f), Some(KeyCode::KEY_F20));
        assert_eq!(hid_usage_to_linux(0xff), None);
    }

    /// Pin a handful of representative `Shortcut -> KeyCombo` rows so an
    /// edit to the table can't silently change what Ctrl+C sends.
    /// `BrowserBack` and `Redo` differ from macOS/Windows by design (see
    /// the module doc on `combo`), so each backend pins its own rows.
    #[test]
    fn combo_table_pins_representative_shortcuts() {
        assert_eq!(combo(Shortcut::Copy).rendered_label(), "Ctrl+C");
        assert_eq!(combo(Shortcut::Redo).rendered_label(), "Ctrl+Shift+Z");
        assert_eq!(combo(Shortcut::BrowserBack).rendered_label(), "Alt+Left");
        assert_eq!(combo(Shortcut::NextTab).rendered_label(), "Ctrl+Tab");
        // hid_usage_to_linux must actually resolve every table entry, or a
        // `Shortcut` silently no-ops instead of pressing anything (see
        // `press_combo`'s warn-and-drop path). Iterates `Shortcut::ALL`
        // rather than a hand-copied list, so a newly added `Shortcut`
        // variant is checked here automatically instead of depending on
        // someone remembering to extend a second, independent list.
        for &shortcut in Shortcut::ALL {
            let key = combo(shortcut).key().code();
            assert!(
                hid_usage_to_linux(key).is_some(),
                "{shortcut:?} table entry has no Linux keycode mapping"
            );
        }
    }

    /// Pin the Hyprland helper table: each NativeAction must map to the
    /// exact `omarchy-*`/`hyprctl` argv the compositor expects, and
    /// Screenshot vs CaptureRegion must stay distinct (upstream mapped both
    /// to Print). Sleep intentionally has no mapping (logind handles it).
    #[test]
    fn hyprland_table_pins_helper_argv() {
        use NativeAction::*;
        let table = [
            (
                PreviousWorkspace,
                "hyprctl",
                &["eval", "hl.dispatch(hl.dsp.focus({workspace='e-1'}))"][..],
            ),
            (
                NextWorkspace,
                "hyprctl",
                &["eval", "hl.dispatch(hl.dsp.focus({workspace='e+1'}))"][..],
            ),
            (LockScreen, "omarchy-system-lock", &[][..]),
            (
                Screenshot,
                "omarchy-capture-screenshot",
                &["fullscreen"][..],
            ),
            (CaptureRegion, "omarchy-capture-screenshot", &["region"][..]),
            (
                FormerWorkspace,
                "hyprctl",
                &["eval", "hl.dispatch(hl.dsp.focus({workspace='previous'}))"][..],
            ),
            (OmarchyMenu, "omarchy-menu", &["toggle"][..]),
            (AppsMenu, "omarchy-menu", &["toggle", "apps"][..]),
            (
                ToggleScratchpad,
                "hyprctl",
                &[
                    "eval",
                    "hl.dispatch(hl.dsp.workspace.toggle_special('scratchpad'))",
                ][..],
            ),
        ];
        for (action, program, args) in table {
            assert_eq!(hyprland_command(action), Some((program, args)));
        }
        assert_eq!(hyprland_command(Sleep), None);
        // Screenshot and region capture must not collapse to one command.
        assert_ne!(
            hyprland_command(Screenshot),
            hyprland_command(CaptureRegion)
        );
    }

    /// A helper's exit status decides between "handled" and the legacy
    /// fallback; a missing helper falls back too.
    #[test]
    fn helper_exit_status_decides_fallback() {
        assert!(run_helper("true", &[]));
        assert!(!run_helper("false", &[]));
        assert!(!run_helper("omalogi-no-such-helper", &[]));
    }

    /// A long-running helper (Omasnap's preview) counts as launched once the
    /// wait expires instead of stalling the action worker until it exits.
    #[test]
    fn long_running_helper_does_not_block_the_worker() {
        let started = std::time::Instant::now();
        assert!(run_helper("sleep", &["30"]));
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    /// A user command that backgrounds a long-lived child returns as soon as
    /// the shell exits: stdio is not piped, so the grandchild cannot hold the
    /// command thread open.
    #[test]
    fn user_command_does_not_wait_for_backgrounded_children() {
        let started = std::time::Instant::now();
        run_user_command("test", "sleep 30 & true");
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    /// One failing action raises one notification per interval; another
    /// action is throttled independently.
    #[test]
    fn failure_notifications_are_rate_limited_per_action() {
        let throttle = FailureThrottle::new();
        let t0 = Instant::now();
        assert!(throttle.should_notify("Omarchy Menu", t0));
        assert!(!throttle.should_notify("Omarchy Menu", t0 + Duration::from_secs(1)));
        assert!(throttle.should_notify("Apps Menu", t0 + Duration::from_secs(1)));
        assert!(throttle.should_notify("Omarchy Menu", t0 + FAILURE_NOTIFY_INTERVAL));
    }
}
