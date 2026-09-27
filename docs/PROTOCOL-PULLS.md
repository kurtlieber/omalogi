# Pulling from upstream

Omalogi tracks `AprilNEA/OpenLogi` as the `upstream` remote and merges each
upstream **release tag** (ADR-0004). The fork shares upstream's full history,
so an ordinary merge brings in every change to code Omalogi has not touched;
only files Omalogi deleted or edited need attention.

## Ownership

**Upstream-pulled** — take upstream's version; do not edit here except for the
listed deltas:

- `crates/openlogi-hidpp/`, `crates/openlogi-hidpp-derive/`
- `crates/openlogi-device/`, `crates/openlogi-device-registry/`
- `crates/openlogi-core/` — delta: `src/brand.rs` (`APP_NAME` and the
  repository/help/release URLs)
- `crates/openlogi-fixture/`
- `crates/openlogi-hid/` (keeps upstream's macOS/Windows transport code)
- `crates/openlogi-ipc/`, `crates/openlogi-agent-core/`
- `crates/openlogi-cli/`, `crates/openlogi/`
- `crates/openlogi-ui/` — delta: the product name in `locales/*.toml`; after
  a merge, re-run `sed -i 's/OpenLogi/Omalogi/g' crates/openlogi-ui/locales/*.toml`
- `crates/openlogi-assets/` — delta: `src/http.rs` (User-Agent names Omalogi)

**Omalogi-owned** — Linux-only rewrites; keep ours and port upstream's
Linux-relevant fixes by hand:

- `crates/openlogi-hook/`, `crates/openlogi-inject/`, `crates/openlogi-agent/`,
  `crates/openlogi-desktop/`, `crates/openlogi-overlay/`,
  `crates/openlogi-camera/`, `crates/openlogi-permissions/`, `xtask/`
- `README.md`, `docs/`, `.github/`, `.agents/`, `AGENTS.md`, `packaging/`,
  `assets/`, `devenv.nix`, `.cargo/config.toml`

## Procedure

```sh
git fetch upstream --tags
git switch -c pull/upstream-vX.Y.Z master
git merge --no-ff --no-commit vX.Y.Z

# 1. Files Omalogi deleted that upstream changed: keep them deleted.
git status --porcelain | awk '$1 == "DU" { print $2 }' | xargs -r git rm -q

# 2. New upstream files for the removed platforms (macOS/Windows backends,
#    bundling, signing, release workflows) arrive as clean additions.
#    Review the list and `git rm` what belongs to a deleted platform.
git diff --cached --name-only --diff-filter=A

# 3. Resolve the remaining conflicts by the ownership table above. For an
#    Omalogi-owned file, read upstream's change (`git log -p ORIG_HEAD..vX.Y.Z
#    -- <path>`) and port only what applies on Linux.

# 4. Prove the tree.
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask ci
```

Commit as `chore: merge upstream vX.Y.Z` and note the tag in `CHANGELOG.md`.

## Shims

When upstream changes an API that an upstream-pulled crate calls on an
Omalogi-owned crate (e.g. `openlogi-agent-core` → `openlogi-hook` /
`openlogi-inject`), add the shim on the Omalogi side — the way
`openlogi_hook::frontmost_safari_pid` and `openlogi_inject::ax_navigate_browser`
remain as Linux no-ops — rather than editing the pulled crate.
