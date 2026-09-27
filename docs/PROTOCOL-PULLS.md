# Protocol pulls from upstream

Omalogi tracks `AprilNEA/OpenLogi` as the `upstream` remote and pulls
**protocol crates only** on upstream release tags (see ADR-0003).

## Pull paths (in scope)

- `crates/openlogi-hidpp/`
- `crates/openlogi-hidpp-derive/`
- `crates/openlogi-device/`
- `crates/openlogi-device-registry/`
- `crates/openlogi-core/`
- `crates/openlogi-fixture/` (test fixtures for the above)

Everything else (`hook`, `inject`, `agent`, `desktop`, `overlay`,
`camera`, `hid`, `permissions`, `xtask`, docs) is Omalogi-owned and never
merged from upstream.

## Procedure

```sh
git fetch upstream --tags
# inspect first: what touched the protocol paths since our last pull?
git log --oneline <last-pull-tag>..upstream/<new-tag> \
  -- crates/openlogi-hidpp crates/openlogi-device \
     crates/openlogi-device-registry crates/openlogi-core \
     crates/openlogi-fixture
# merge just those paths (ours wins everywhere else by construction —
# those trees don't exist upstream in conflicting form):
git merge -X subtree=crates/openlogi-hidpp upstream/<new-tag>  # repeat per path,
# …or cherry-pick the protocol commits listed by the log above.
cargo test -p openlogi-inject -p openlogi-hook -p openlogi-device \
  -p openlogi-core -p openlogi-hidpp
```

Record the pulled tag in `CHANGELOG.md`.

## Conflict policy

- Conflict inside a pull path: resolve in favor of upstream, then re-apply
  any Omalogi delta (there should be none — we don't touch these crates).
- Upstream commit touching pull paths *and* shell crates: take the pull-path
  hunks only (`git checkout upstream/<tag> -- <pull paths>` for those
  files, review the diff, commit).
- A protocol change that alters the `Effect` IR seam: write the shim in
  `inject/linux.rs`, note it in the changelog, and pin it with a test next
  to `hyprland_table_pins_helper_argv`.
