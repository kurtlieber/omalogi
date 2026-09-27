---
paths:
  - "crates/openlogi-hook/**"
  - "crates/openlogi-inject/**"
  - "crates/openlogi-hid/**"
  - "crates/openlogi-agent/src/autostart.rs"
  - "crates/openlogi-agent/src/autostart/**"
  - "crates/openlogi-camera/**"
  - "crates/openlogi-permissions/**"
---

# Platform / cfg-gated code — Linux only

Omalogi targets Linux (Omarchy/Hyprland) alone. Two rules follow:

- **Omalogi-owned crates carry no macOS/Windows code.** Do not add
  `#[cfg(target_os = "macos")]` / `windows` branches, target-specific
  dependencies, or platform stubs for them in `openlogi-hook`, `-inject`,
  `-agent`, `-desktop`, `-overlay`, `-camera`, `-permissions`, or `xtask`.
  A `cfg(not(target_os = "linux"))` fallback is fine where it keeps a portable
  crate compiling.
- **Upstream-pulled crates keep upstream's other-OS code untouched**
  (`openlogi-hid`, `openlogi-agent-core`, and the protocol crates — see
  `docs/PROTOCOL-PULLS.md`). Editing it only creates conflicts on the next pull.
  If upstream changes a shared API these crates expose, shim it on the Omalogi
  side (e.g. `openlogi_hook::frontmost_safari_pid` stays as an always-`None`
  function because `openlogi-agent-core` calls it).

`RUSTFLAGS=-D warnings` is global in CI — plain warnings fail there too.
