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
- `crates/openlogi-core/` — deltas: `src/brand.rs` (`APP_NAME`, the
  repository/help/release URLs, and the `omalogi` executables and `APP_ID`,
  ADR-0006); `src/paths.rs` (`APP_DIR = "omalogi"` and
  `adopt_legacy_dirs`, ADR-0006); the Omarchy action vocabulary and
  `[commands]` table (ADR-0005): `src/binding/{action,effect,defaults}.rs`,
  `src/binding/action_ring/icon.rs` (label keys), `src/config.rs` (the
  `commands` field), `src/config/commands.rs`
- `crates/openlogi-fixture/`
- `crates/openlogi-hid/` (keeps upstream's macOS/Windows transport code)
- `crates/openlogi-ipc/`
- `crates/openlogi-agent-core/` — delta: `src/runtime/pointer.rs` (no
  focus gate for `FormerWorkspace`)
- `crates/openlogi-cli/` — deltas: `src/cmd/reload.rs` (`omalogi reload`);
  `src/lib.rs` (clap name `omalogi` and the `adopt_legacy_dirs` call,
  ADR-0006)
- `crates/openlogi/` — delta: `Cargo.toml` (`[[bin]] name = "omalogi"`,
  ADR-0006)
- `crates/openlogi-ui/` — deltas: the product name in `locales/*.toml`, and
  the renamed Navigation keys (`actions.omarchy_menu`, …,
  `pointer.previous_next_workspace`); after a merge, re-run
  `sed -i 's/OpenLogi/Omalogi/g' crates/openlogi-ui/locales/*.toml`
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

# 4. Upstream code that names a renamed action (`Action::MissionControl`,
#    `NativeAction::ShowDesktop`, `actions.launchpad`, …) no longer compiles
#    or finds its locale key. Rename it with the ADR-0005 table. Expected
#    hits: the `serde(alias)` lines in `action.rs`, the icon names beside
#    them, and the tests that load upstream names.
git grep -nE 'MissionControl|AppExpose|LaunchpadShow|PreviousDesktop|NextDesktop|ShowDesktop|mission_control|app_expose|launchpad|show_desktop' \
  -- crates ':!crates/openlogi-hidpp' ':!*/action_ring/icon.rs'

# 5. Prove the tree.
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
