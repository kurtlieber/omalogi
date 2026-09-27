# Developing Omalogi

This document covers the local development workflow for Omalogi, the
Omarchy/Hyprland fork of [OpenLogi](https://github.com/AprilNEA/OpenLogi). For
end-user install instructions, see [INSTALL-linux.md](INSTALL-linux.md).

## Toolchain

- Stable Rust (Edition 2024, MSRV 1.98 — the floor tracks current stable)
- Linux system libraries for GPUI, udev, and TLS. On Debian/Ubuntu (what CI
  uses): `sudo apt-get install libudev-dev gcc g++ clang libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev pkg-config`.
  On Arch/Omarchy install the equivalent packages; a missing library fails the
  build with the pkg-config name to look for.
- [nfpm](https://nfpm.goreleaser.com/) for `cargo xtask linux package`.

## Building from source

Nix/devenv is optional. A normal Rust toolchain is enough.

### Without Nix

```sh
# rustup installs the stable toolchain pinned in rust-toolchain.toml
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
git clone https://github.com/kurtlieber/omalogi
cd omalogi
cargo run -p openlogi --release -- list
cargo run -p openlogi-desktop --release
```

If you use [direnv](https://direnv.net) without devenv installed, `.envrc`
prints a notice and leaves your shell alone. Install rustup/cargo yourself
and keep working.

### With devenv (optional)

`devenv.nix` provisions sccache, the stable Rust toolchain, the Linux
libraries, and nfpm. Tasks:

```sh
devenv tasks run openlogi:gui      # run the desktop app
devenv tasks run openlogi:check    # fmt + clippy + tests + rustdoc
devenv tasks run openlogi:ci       # every GitHub Actions CI job this host can reproduce
```

After a `devenv.nix` change, reload direnv so the new env takes effect
(`direnv reload`, or exit your shell and `cd` back in).

### Nix package

The root Flake exposes native `x86_64-linux` and `aarch64-linux` packages plus
the NixOS module. It is separate from the devenv shell:

```sh
nix flake check --all-systems --no-build  # evaluate every output
nix build .#openlogi                      # build + test this host's package
nix run .#openlogi -- list                # run the packaged CLI
```

The package expression and NixOS module live beside the other Linux packaging
inputs in `packaging/linux/`. `nix fmt` formats all Nix expressions through the
Flake's pinned formatter.

### Running a dev build

Run the agent and the GUI from two terminals:

```sh
cargo run -p openlogi-agent
cargo run -p openlogi-desktop
```

Each binary holds a single-instance lock, so stop a packaged agent first
(`systemctl --user stop openlogi-agent`) and quit any running GUI.

## Developing the GUI without hardware

`openlogi-agent-mock` serves the real agent IPC contract from a scripted
in-memory inventory, so the desktop app can be developed with no Logitech
device (or receiver) attached:

```sh
cargo run -p openlogi-agent --bin openlogi-agent-mock   # then, in another terminal:
OPENLOGI_PROFILE=dev cargo run -p openlogi-desktop
```

The mock defaults itself to the `openlogi-dev` profile (as if `OPENLOGI_PROFILE=dev`
were set), so it meets a GUI started with `OPENLOGI_PROFILE=dev` on the dev
socket, and an installed agent, which is on the production profile, keeps
running untouched. Pass `OPENLOGI_PROFILE=prod` to serve the production socket
instead; the mock then contends for the production agent's single-instance
lock and refuses to start while it is running.

The script covers an online mouse (DPI and SmartShift writes persist and read
back, battery drains so poll-driven repaints are visible), an offline mouse, a
lighting-capable keyboard, a directly-attached device, and a full Bolt pairing
flow (discovery → passkey → paired). Its agent version carries a `-mock` suffix,
so a mock session is identifiable in the UI. It is a dev tool only and is never
bundled.

The proposed architecture for recorded device profiles, raw HID++ replay, and
deterministic hardware scenarios is documented in
[Mock device and hardware record/replay architecture](MOCK_DEVICE_TESTING.md).

### Component gallery

Use the debug-only component gallery to review shared controls across light and
dark themes and every supported interface scale without config, IPC, or hardware:

```sh
OPENLOGI_COMPONENT_GALLERY=1 cargo run -p openlogi-desktop
```

Gallery mode opens one isolated window and bypasses the normal single-instance,
config, agent, asset-sync, and updater startup paths. The environment variable is
ignored by release builds.

## Project layout

```
crates/
  openlogi/         the `openlogi` binary — a thin wrapper over openlogi-cli
  openlogi-core/    types, config (TOML), paths, button + action catalog — no HID, no async
  openlogi-inject/  OS input synthesis: uinput/MPRIS, plus the Hyprland/Omarchy helper table
  openlogi-hidpp/   vendored HID++ protocol crate (lib name `hidpp`)
  openlogi-hid/     device discovery, HID++ reads/writes, and control capture over async-hid
  openlogi-assets/  device-render registry schema + cached HTTP fetch from OpenLogi asset mirrors
  openlogi-cli/     CLI implementation: command tree + `run()`, called by the `openlogi` binary
  openlogi-agent-core/  shared orchestration + the agent/GUI IPC contract
  openlogi-agent/   the `openlogi-agent` binary — background agent owning device I/O and the hook
  openlogi-hook/    OS mouse hook: evdev grab + uinput re-injection
  openlogi-ui/      presentation shared by the two GPUI processes: ring geometry/icons,
                    the GPUI asset source, locale negotiation — gpui, no gpui-component
  openlogi-desktop/     the `openlogi-desktop` binary — GPUI + gpui-component IPC client
  openlogi-overlay/ the `openlogi-overlay` binary — the cursor-centred Actions Ring
```

## Agent guidance

Shared rules have one tracked source: [`.agents/rules/`](../.agents/rules/).
Edit and link those `.md` files, not a client-specific copy. The tracked
`.claude/rules` symlink points to `../.agents/rules` inside the checkout.
Existing references to `.claude/rules/<name>.md` still resolve through that alias.
Crate-specific contracts stay in each crate's `AGENTS.md`; task workflows stay
in `.agents/skills/`. No rule generator or install step is required.

The root [AGENTS.md](../AGENTS.md) holds global instructions and the rule index.
`CLAUDE.md` imports only that entrypoint. Claude Code discovers `.md` rules
through `.claude/rules` and uses their `paths` metadata for conditional loading.
Other clients must follow the index; `.agents/rules/` is not a universal
automatic discovery path. Keep the index as ordinary Markdown links: importing
every rule from the root would load unrelated guidance into Claude's context.

### Check the checkout and client loading

From the repository root, in a POSIX shell or Git Bash:

```sh
git ls-files --stage -- .claude/rules .agents/rules
test -L .claude/rules && test -f .claude/rules/rust.md
readlink .claude/rules
```

Expect regular rule files under `.agents/rules/` and one `120000` entry
for `.claude/rules`, whose link target is `../.agents/rules`. The file checks
must succeed too: the index mode alone does not prove a working symlink.

Verify loading in the client, not only the filesystem. In a fresh Claude Code
session, use `/context` or an `InstructionsLoaded` hook to inspect loaded files:

- Read `README.md`: Rust and GUI path rules should not load solely from that read.
- Read `crates/openlogi-core/src/lib.rs`: the Rust rule should load, but not GUI.
- Read `crates/openlogi-desktop/src/app.rs`: the GUI rule should now load too.

Follow [Claude's loading diagnostics](https://code.claude.com/docs/en/memory#troubleshoot-memory-issues)
if those results differ. Filesystem checks and unchanged `paths` metadata are
not evidence that a particular client version loaded the rules correctly.

## Local CI

The PR test pipeline is `.github/workflows/ci.yml`. To run every job this
machine can reproduce — including typos, the ast-grep guards, MSRV, and
cargo-deny, which the host gate does not run:

```sh
cargo xtask ci
cargo xtask ci --list                        # job → command table
devenv tasks run openlogi:ci                 # same, from devenv
```

The runner sets `RUSTFLAGS=-D warnings` the way CI does. A job this host cannot
run is reported as skipped; a skip is not a pass. The full job map (and which
diff requires which job) is [`.agents/rules/ci.md`](../.agents/rules/ci.md).

### Pre-push gate

Before pushing, read [the local gate and push checklist](../.agents/rules/ci.md#local-gate-hard-stop-before-push--scale-it-to-the-affected-graph).
That file owns tier selection, exact commands, and additional checks required by
the diff. `devenv tasks run openlogi:check` runs the full host tier, not the
whole CI pipeline. A Rust-bearing rebase or conflict resolution requires the
full tier. Non-Rust changes use the applicable non-Rust checks.

## GitHub workflow

- Conventional commits (`type(scope): description`); see the root `AGENTS.md`.
- Upstream protocol updates arrive through the procedure in
  [PROTOCOL-PULLS.md](PROTOCOL-PULLS.md), never a wholesale merge.
- These procedures do not authorize remote writes: get approval before pushing,
  opening or merging PRs, or publishing.

## Packaging Linux `.deb` / `.rpm` / `.pkg.tar.zst`

Requires [nfpm](https://nfpm.goreleaser.com/) on `PATH`; the package arch is
derived from the host (override with `PKG_ARCH`):

```sh
cargo run -p xtask -- linux package
# → target/release/*.deb / *.rpm / *.pkg.tar.zst
```

The package contents (binaries, udev rules, systemd user unit, desktop entry,
icon) are declared in `packaging/linux/nfpm.yaml`.

The Nix package uses the same shared resources and is declared in
`packaging/linux/package.nix`; see the Nix package section above for its build
commands.

## Installation-source detection

The desktop app probes once in the background and publishes the typed
`platform::installation::Installation` global: `Detecting`, then
`Detected(InstallationSource)`. It also logs `detected installation source`.
Settings → Updates displays the result as **Installation source**, separately
from the update download source. An open window refreshes when detection
completes or the interface language changes.
This is an ownership snapshot, not download provenance or an update policy;
the updater does not yet change behavior based on it.

- **Linux:** recognizes a resolved `/nix/store/` executable, or queries dpkg,
  rpm, and pacman for ownership of the exact executable by `openlogi`.
  Package queries are read-only, with a two-second timeout per command.
- **Unknown:** bare source/manual installs, or otherwise inconclusive ownership.

## Updater

The desktop app's opt-in update check reads a static manifest at
`OPENLOGI_UPDATE_MANIFEST_URL` (default: this repository's
`releases/latest/download/latest.json`) and installs only artifacts whose
minisign signature verifies against `OPENLOGI_UPDATE_MINISIGN_PUBLIC_KEY`,
embedded at build time. Omalogi publishes no signed releases or manifest yet,
so builds carry no key and the check fails closed.
