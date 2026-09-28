# ADR-0001: Keep `openlogi-*` crate names

Status: accepted

## Context

Omalogi rebrands OpenLogi for Omarchy. Renaming all 20 crates to `omalogi-*`
would touch every `Cargo.toml` and every `use` line.

## Decision

Keep upstream crate/lib names (`openlogi-core`, `openlogi-inject`, …).
Rebrand display strings, README, and docs only.

ADR-0006 narrows this: executables, the systemd unit, config paths, and the
package are `omalogi`; only crate names and code identifiers stay upstream's.

## Consequences

- Upstream protocol commits cherry-pick with near-zero conflicts.
- `cargo` output and log targets still say `openlogi`; accepted as cosmetic.
- Revisit only if crates are ever published to crates.io under our name.
