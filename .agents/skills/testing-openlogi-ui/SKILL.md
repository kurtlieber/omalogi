---
name: testing-openlogi-ui
description: "Verifies OpenLogi native UI with focused GPUI tests, the component gallery, and mock-agent workflows. Use when changing or testing desktop, overlay, shared controls, theme, scale, localization, or UI interactions."
---

# Test OpenLogi UI

Choose evidence that proves the affected behavior, then inspect the rendered result.

## Choose the test surface

1. Read [GUI integration rules](../../../.agents/rules/gui.md) and load
   [gpui-kit](../gpui-kit/SKILL.md). For visual or interaction changes, also load
   [gpui-kit-design-guides](../gpui-kit-design-guides/SKILL.md).
2. Name the expected result and a failing case before editing. Use pure tests for
   logic, GPUI tests for rendered interactions, and the running app for OS behavior.
3. Use the gallery for reusable controls; update the affected gallery entry.
   Use the mock agent for device panels and IPC-driven state without hardware.
   Test the overlay process separately; a desktop ring preview does not verify it.
4. Read [development setup](../../../docs/DEVELOPMENT.md) before launching.
   Check the available native display and build tools. Browser DOM automation does
   not test GPUI. If native rendering is blocked, report the blocker and run the
   strongest available tests instead.

## Run without hardware

Run from the repository root.

For isolated controls in a debug build:

```sh
OPENLOGI_PROFILE=dev OPENLOGI_COMPONENT_GALLERY=1 cargo run -p openlogi-desktop
```

For app workflows, start the mock first. After it binds its endpoint, start the
desktop in a second terminal or managed service:

```sh
OPENLOGI_PROFILE=dev cargo run -p openlogi-agent --bin openlogi-agent-mock
OPENLOGI_PROFILE=dev cargo run -p openlogi-desktop
```

- Keep all test processes on the same profile. Do not use `prod` to bypass a lock.
  Identify a conflicting process before stopping it; preserve installed apps.
- Quit the previous dev GUI before relaunching; a second instance exits on the
  singleton lock.
- Confirm the connected agent has the `-mock` version suffix before changing settings.
  If it disconnects, restore the mock before continuing; the GUI can auto-start helpers.
- For repeatable captured inventory, append `-- --fixture <profile.json>` to the
  mock command. Fixture mode freezes mock time; demo mode animates battery/pairing.
  Do not wait for demo pairing transitions in fixture mode.

## Verify the affected contract

Use the installed GPUI API, not unverified upstream test helpers. The
[gallery smoke test](../../../crates/openlogi-desktop/src/ui/gallery.rs),
[picker interaction test](../../../crates/openlogi-desktop/src/features/profiles/picker.rs),
and [ring scroll test](../../../crates/openlogi-desktop/src/features/action_ring/editor.rs)
show working local harnesses. Run the affected test, for example:

```sh
cargo test -p openlogi-desktop gallery_renders_without_application_state
```

- Assert an interaction's visible result and owner state, not just successful render.
  Exercise the relevant negative state: disabled control, offline device, invalid
  input, empty list, or IPC failure. Do not manufacture test-only state to pass.
- For visible changes, render representative affected states, capture screenshots,
  and inspect them. Check affected light/dark themes, supported interface scales,
  narrow layouts, and open menus or dialogs. A smoke test is not pixel verification.
- For localized UI, change language while the view is open and inspect cached text.
  Apply the catalog and wiring checks in [i18n rules](../../../.agents/rules/i18n.md).
- Apply [change verification](../verifying-openlogi-changes/SKILL.md) once stable.
  CI's Linux test job excludes the desktop crate; run `cargo test -p openlogi-desktop`
  locally.

Report commands and results, exercised states, and an inspected screenshot for
visual work. Separate in-process tests, running-app checks, and real-hardware checks.
Mock success does not prove HID writes, OS permissions, input hooks, or physical
device behavior. Name every relevant OS or hardware check that was not run.
