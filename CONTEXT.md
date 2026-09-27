# Omalogi — Ubiquitous Language

Glossary only. No implementation details.

- **Omalogi**: this project. An Omarchy/Hyprland-only Logitech remapper.
  Hard fork of OpenLogi; Linux is the only platform.
- **Upstream**: `AprilNEA/OpenLogi`, tracked as the `upstream` git remote.
  Source of all HID++ protocol and device support.
- **NativeAction**: OpenLogi's platform-neutral window-manager/power action
  enum (`MissionControl`, `AppExpose`, `PreviousDesktop`, `NextDesktop`,
  `ShowDesktop`, `LaunchpadShow`, `LockScreen`, `Screenshot`,
  `CaptureRegion`, `Sleep`). Defined in `openlogi-core`; unchanged by Omalogi.
- **Omarchy action**: the concrete Hyprland/Omarchy command a NativeAction
  runs as (`hyprctl dispatch …`, `omarchy-system-lock`,
  `omarchy-capture-screenshot`, `omarchy-menu toggle`).
- **Gesture button**: the physical Logitech button assigned the gesture role
  (on the M720 Triathlon, the button behind the scroll wheel). Press-and-move
  produces directional gestures; a plain click is the Click action.
- **Upstream-pulled crates**: the protocol, device, transport, IPC, and
  shared-agent crates (`openlogi-hidpp`, `openlogi-device`,
  `openlogi-device-registry`, `openlogi-core`, `openlogi-hid`,
  `openlogi-ipc`, `openlogi-agent-core`, …). Hardware knowledge lives here.
  Merged from upstream release tags; never forked divergently.
- **Omalogi-owned crates**: everything that touches the OS or screen
  (`openlogi-inject`, `openlogi-hook`, `openlogi-agent`, desktop, overlay,
  camera, permissions). Linux-only; upstream changes are ported by hand.
- **Helper**: an Omarchy/`hyprctl` executable invoked with fixed argv and no
  shell (`hyprland_command` table in `inject/linux.rs`).
- **Legacy chord**: the pre-fork GNOME/KDE key synthesis
  (`Ctrl+Alt+Left/Right`, `Print`, logind). Fallback when a helper is
  missing or fails; behavior off Hyprland.
