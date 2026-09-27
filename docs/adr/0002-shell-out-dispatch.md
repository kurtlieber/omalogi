# ADR-0002: Shell-out dispatch over Hyprland IPC socket

Status: accepted

## Context

NativeActions need Omarchy behavior. Options: (a) speak Hyprland's IPC
socket directly, (b) shell out to `hyprctl` / `omarchy-*` helpers with fixed
argv. Upstream PRs #1152/#1162 chose (b).

## Decision

Shell out with fixed argv, no shell, resolved via `PATH`. Fall back to the
legacy chord when the helper is missing or fails. No logind attempt for
LockScreen on Hyprland (`LockSession` OK ≠ hyprlock ran).

## Consequences

- Covers `omarchy-*` helpers that have no IPC equivalent
  (lock, screenshot, menu) — a socket client couldn't.
- Matches upstream PRs, so those cherry-pick if merged.
- Synchronous on the action worker; a hung helper stalls later remaps
  (marked `ponytail:` in code — sidecar thread if it bites).
- Helpers must exist on `PATH`; FHS-standard install paths keep this true
  for AUR packaging (phase 2).
