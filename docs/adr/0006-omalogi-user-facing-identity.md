# ADR-0006: Omalogi's user-facing identity

Status: accepted (narrows ADR-0001)

## Context

ADR-0001 kept upstream's `openlogi-*` crate names so upstream merges stay
clean, and everything a user touches inherited them: the `openlogi` command,
`openlogi-agent.service`, `~/.config/openlogi`, the `openlogi` package. Users
of Omalogi were typing another project's name, and the package name collided
with upstream's (`openlogi-bin` is in the AUR), which a submission to the
Omarchy package repository cannot do.

Crate names are code; command, unit, path, and package names are interface.
Only the first needs to match upstream for merges to stay cheap.

## Decision

1. **Crates keep `openlogi-*`** (ADR-0001), and so do the Rust identifiers,
   the `OPENLOGI_*` environment variables, and log targets.
2. **Everything a user types, installs, or configures is `omalogi`:**
   - executables `omalogi`, `omalogi-desktop`, `omalogi-agent`,
     `omalogi-overlay` (`[[bin]]` names; `brand::CLI_EXECUTABLE`,
     `GUI_EXECUTABLE`, and `Helper::executable` match);
   - `omalogi-agent.service`, `70-omalogi.rules`, `omalogi.desktop`, the
     `omalogi` icon, `/usr/share/licenses/omalogi/`;
   - `~/.config/omalogi`, `~/.local/share/omalogi`, `~/.local/state/omalogi`;
   - the Wayland `app_id` `omalogi` (`brand::APP_ID`) and
     `omalogi-action-ring` for the overlay, which Hyprland window rules match;
   - the built-in themes, "Omalogi Light" and "Omalogi Dark";
   - the package `omalogi`, and the Nix package, NixOS module
     (`programs.omalogi`), and flake outputs.
3. **The package conflicts with `openlogi`, `openlogi-bin`, `openlogi-git`,
   and `solaar`.** Only one HID++ manager can own a receiver.
4. **Existing installs carry over.**
   - Every binary calls `openlogi_core::paths::adopt_legacy_dirs()` first
     thing: an `openlogi` config, data, or state directory is renamed to
     `omalogi` when no `omalogi` directory exists yet.
   - The agent disables and removes an `openlogi-agent.service` it generated
     (recognised by exact match against the old template), so only the
     renamed unit starts an agent.
   - `install.sh` and `uninstall.sh` remove the old `openlogi` binaries, unit,
     udev rule, desktop entry, and icons, but only when the old desktop entry
     says `Name=Omalogi`, so an upstream OpenLogi install is never touched.
   - A saved theme name of "OpenLogi Light" or "OpenLogi Dark" no longer
     matches and falls back to the default, which is the same theme renamed.

## Consequences

- Upstream-pulled files gain small deltas, listed in PROTOCOL-PULLS.md:
  `crates/openlogi/Cargo.toml` (`[[bin]] omalogi`),
  `openlogi-cli/src/lib.rs` (clap name and the legacy-directory call),
  `openlogi-core/src/paths.rs` (`APP_DIR` and the adoption), and
  `openlogi-core/src/brand.rs` (already a delta).
- `cargo build -p openlogi-agent` builds `target/release/omalogi-agent`:
  package names and executable names now differ.
- The macOS-only identifiers in `brand.rs` (`AGENT_ID`, the launchd label)
  and the `openlogi://` deep-link scheme stay upstream's; nothing on Linux
  shows them.
- A user who later reinstalls upstream OpenLogi starts from a fresh
  `~/.config/openlogi`, because Omalogi moved the old one.
