<h1 align="center">Omalogi</h1>

<p align="center"><strong>Logitech mouse and keyboard control for <a href="https://omarchy.org/">Omarchy</a> — where the gesture button, workspace keys, lock, and screenshot run real Hyprland commands.</strong></p>

> [!WARNING]
> **Omalogi is early and personal.** It is a single-maintainer fork, built from
> source, with no published packages yet. Expect config and behaviour to change.

Omalogi is a Linux-only fork of [**OpenLogi**](https://github.com/AprilNEA/OpenLogi)
by [@AprilNEA](https://github.com/AprilNEA). OpenLogi does the hard part — the
HID++ protocol, device support, the agent, and the GUI. Omalogi narrows it to one
desktop, Omarchy on Hyprland, and makes the system actions do what an Omarchy user
expects:

- **Workspace buttons and gestures** switch Hyprland workspaces, or jump back
  to the one you were just on.
- **Omarchy Menu** and **Apps Menu** open `omarchy-menu`.
- **Toggle Scratchpad** shows and hides the scratchpad (SUPER+S).
- **Lock** runs `omarchy-system-lock`.
- **Screenshot** and **Capture Region** run `omarchy-capture-screenshot`.
- **Your own scripts** can replace any of these (see [Custom commands](#custom-commands)).

Everything Hyprland-specific is a small, tested table in
`crates/openlogi-inject/src/inject/linux.rs`. The macOS and Windows backends
were removed, so the code that remains is the code that runs on your machine.

## What you get

Inherited from OpenLogi and working on Omarchy:

- Logi Bolt and Unifying receivers, Bluetooth, and wired devices, with battery
  level and charge state
- Button remapping with a built-in action catalog, custom shortcuts, separate
  short/long-press actions, and hold-until-release chords
- Per-direction gestures on the gesture button (the button behind the wheel on
  an M720 Triathlon, the thumb button on an MX Master)
- DPI presets, SmartShift, and scroll inversion on supported mice
- The Actions Ring — a cursor-centred, eight-slot action overlay
- Per-application profiles keyed by Hyprland's Wayland `app_id`
- Static RGB lighting on supported keyboards, Litra light control, and UVC
  webcam controls
- A plain TOML config file and a CLI alongside the GUI

## Hyprland action mapping

| Button action | Command | Omarchy key |
|---|---|---|
| Omarchy Menu | `omarchy-menu toggle` | SUPER+SPACE |
| Apps Menu | `omarchy-menu toggle apps` | SUPER+ALT+SPACE |
| Former Workspace | `hyprctl eval` → `hl.dsp.focus({workspace='previous'})` | SUPER+CTRL+TAB |
| Previous / Next Workspace | `hyprctl eval` → `hl.dsp.focus({workspace='e-1' / 'e+1'})` | SUPER+(SHIFT+)TAB |
| Toggle Scratchpad | `hyprctl eval` → `hl.dsp.workspace.toggle_special('scratchpad')` | SUPER+S |
| Lock Screen | `omarchy-system-lock` | |
| Screenshot | `omarchy-capture-screenshot fullscreen` | |
| Capture Region | `omarchy-capture-screenshot region` | |
| Sleep | logind `Suspend` | |

Helpers run with fixed arguments and no shell. If a helper is missing or
fails, previous/next workspace, lock, and screenshots fall back to the generic
GNOME/KDE key chords. Off Hyprland (no `HYPRLAND_INSTANCE_SIGNATURE`), those
chords are all you get.

These six Navigation actions are OpenLogi's macOS actions (Mission Control,
App Exposé, Launchpad, …) renamed for Omarchy. An OpenLogi config still loads;
see [ADR-0005](docs/adr/0005-omarchy-action-vocabulary.md).

## Custom commands

Point any built-in action at your own script with a `[commands]` table in
`~/.config/openlogi/config.toml`. The override applies everywhere that action
is bound:

```toml
[commands]
OmarchyMenu = "~/bin/my-overview"
VolumeUp = "pamixer -i 2"
```

Run `openlogi reload` to apply the edit. If a command fails, you get a
desktop notification and the built-in action does not run. To give just one
button a command, pick **Run Shell Command…** in that button's action list.
Details: [Configuration → Command overrides](docs/CONFIGURATION.md#command-overrides).

## Install

> [!IMPORTANT]
> Quit **Solaar** or **Logi Options+** first. They compete for HID++ access,
> and only one app can own a receiver at a time.

Build from source with a stable Rust toolchain:

```sh
git clone https://github.com/kurtlieber/omalogi
cd omalogi
cargo xtask linux package          # builds and packages; needs nfpm
sudo pacman -U target/release/openlogi-*.pkg.tar.zst
systemctl --user enable --now openlogi-agent.service
```

The package installs udev rules that give your user access to `/dev/hidraw*`,
`/dev/uinput`, and your mouse's input node without `sudo`. Binaries, the
systemd unit, and the config directory keep upstream's `openlogi` names, so
upstream updates keep merging cleanly (see
[ADR-0001](docs/adr/0001-keep-crate-names.md)).

Manual installs, NixOS, and troubleshooting: [docs/INSTALL-linux.md](docs/INSTALL-linux.md).

## Documentation

- [Usage (CLI)](docs/USAGE.md)
- [Configuration](docs/CONFIGURATION.md)
- [Development](docs/DEVELOPMENT.md)
- [Relationship to upstream](docs/PROTOCOL-PULLS.md) — how OpenLogi releases
  are merged, and which parts Omalogi owns

## Relationship to OpenLogi

Omalogi is a hard fork, not a replacement. New devices, protocol fixes, and
most features arrive by merging OpenLogi's release tags. Device-support
requests and HID++ bugs belong
[upstream](https://github.com/AprilNEA/OpenLogi/issues). Omalogi's own issues
cover the Omarchy integration and the Linux-only changes.

If you use macOS or Windows, or a Linux desktop other than Hyprland, use
[OpenLogi](https://github.com/AprilNEA/OpenLogi).

## Acknowledgments

- [OpenLogi](https://github.com/AprilNEA/OpenLogi) by [@AprilNEA](https://github.com/AprilNEA)
  and its contributors — the protocol stack, device support, agent, GUI, and
  nearly all of the code in this repository
- The Linux port of OpenLogi by [@cserby](https://github.com/cserby)
- [Solaar](https://github.com/pwr-Solaar/Solaar) — an open-source HID++
  implementation
- [Omarchy](https://omarchy.org/) — the desktop this fork exists for

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option, like upstream OpenLogi. `crates/openlogi-hidpp` is a vendored
fork of [`hidpp`](https://crates.io/crates/hidpp) by [@lus](https://github.com/lus),
licensed 0BSD.

OpenLogi's name, logo, and brand assets belong to AprilNEA and are not used
here. The Omalogi icon is original.

---

**Not affiliated with Logitech or with the OpenLogi project.** "Logitech",
"MX Master", and "Options+" are trademarks of Logitech International S.A.
