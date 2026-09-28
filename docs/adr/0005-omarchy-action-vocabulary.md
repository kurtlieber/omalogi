# ADR-0005: Omarchy action vocabulary and `[commands]` overrides

Status: accepted (narrows ADR-0004 for `openlogi-core`, `openlogi-ui`,
`openlogi-agent-core`, and `openlogi-cli`)

## Context

Upstream's Navigation actions are macOS concepts: Mission Control, App
Exposé, Launchpad, and "desktops". On Omarchy two of them did nothing
(no overview exists), and the rest ran Hyprland commands under macOS names.
Users also had no way to point a built-in action at their own script short of
rebinding every button to `RunShellCommand` one by one.

ADR-0004 keeps `openlogi-core` upstream-pulled so release merges stay clean.
Fixing the vocabulary means editing it, and so does a config-level override
table. Omalogi will diverge from upstream over time anyway; the question is
whether to pay for it here.

## Decision

1. **Rename the six Navigation actions in place** in `openlogi-core`:

   | Upstream name | Omalogi name | Runs |
   |---|---|---|
   | `MissionControl` | `OmarchyMenu` | `omarchy-menu toggle` |
   | `AppExpose` | `FormerWorkspace` | `hl.dsp.focus({workspace='previous'})` |
   | `PreviousDesktop` | `PreviousWorkspace` | `hl.dsp.focus({workspace='e-1'})` |
   | `NextDesktop` | `NextWorkspace` | `hl.dsp.focus({workspace='e+1'})` |
   | `ShowDesktop` | `ToggleScratchpad` | `hl.dsp.workspace.toggle_special('scratchpad')` |
   | `LaunchpadShow` | `AppsMenu` | `omarchy-menu toggle apps` |

   Each keeps its declaration slot, so the serde variant index (the IPC wire
   format) is unchanged. A `#[serde(alias = "<upstream name>")]` reads older
   and upstream-written configs; the next save writes the Omalogi name. No
   `schema_version` bump, so Omalogi's schema numbering does not collide with
   upstream's next one. `NativeAction` is renamed to match, and the locale
   keys follow (`actions.omarchy_menu`, …) in all 23 languages.

2. **Add a `[commands]` table** to `config.toml` (`CommandOverrides` in
   `openlogi-core`): action name → shell string. It replaces that action
   globally — every device, profile, gesture, F-key, and Actions Ring slot.
   Only one-shot actions (shortcut, media, and native effects) are
   overridable; clicks, scrolls, held chords, and agent-side actions are
   rejected at load with the TOML location. An override is final: a failure
   is logged and raised as a rate-limited `notify-send`, and the built-in
   action does not run. The injector (`openlogi_inject::execute`) applies the
   table; the agent installs it at startup and on every `reload_config`.

3. **`omalogi reload`** (named `openlogi reload` until ADR-0006) asks the running agent to re-read `config.toml`, so a
   hand edit applies without restarting the service.

4. The GUI offers **Run Shell Command…** in the mouse button, gesture, and
   Actions Ring pickers, and no longer offers Run AppleScript. The
   `RunAppleScript` action stays in `openlogi-core` for wire-format stability.

## Consequences

- An upstream merge that touches these actions conflicts, or fails to compile
  where upstream code names `Action::MissionControl` and friends. Resolve by
  renaming to the Omalogi name; the table above is the mapping.
- An upstream change that appends `Action` variants still merges, because
  the renamed variants did not move.
- Configs stay portable in one direction: OpenLogi configs load in Omalogi;
  a config Omalogi has saved does not load in OpenLogi.
- `openlogi-core`, `openlogi-ui` locales, `openlogi-agent-core`
  (`runtime/pointer.rs`), and `openlogi-cli` now carry Omalogi deltas, listed
  in [PROTOCOL-PULLS.md](../PROTOCOL-PULLS.md).
