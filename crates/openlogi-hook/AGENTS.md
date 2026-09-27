# openlogi-hook — OS input capture

Linux only: an exclusive `evdev` grab on the Logitech mouse plus `uinput`
re-injection of everything the agent passes through. Platform discipline is
[`.agents/rules/cross-platform.md`](../../.agents/rules/cross-platform.md); this
file is the crate's own load-bearing behavior.

- Grab only Logitech hardware (`source_is_remappable`, the vendor check in
  `linux.rs`) and never the hook's own virtual devices — a grab on any other
  pointer takes it away from the desktop.
- The event callback must never block: queue bound actions off-thread. A stalled
  callback stalls the grabbed mouse.
- Foreground-app detection picks one source per process (`linux/foreground.rs`):
  wlr-foreign-toplevel on Hyprland (Wayland `app_id`), the GNOME Shell extension,
  or X11 (`WM_CLASS`). The two namespaces do not map onto each other; profiles
  match by exact string.
- `frontmost_safari_pid` stays as an always-`None` function because the
  upstream-pulled `openlogi-agent-core` calls it; keep such shims rather than
  editing the pulled crate.
