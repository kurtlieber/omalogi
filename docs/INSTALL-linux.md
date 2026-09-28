# Installing Omalogi

Omalogi targets [Omarchy](https://omarchy.org/) (Arch Linux + Hyprland). It
builds and runs on other Linux distributions, but system actions only map to
native commands on Hyprland.

> [!NOTE]
> HID++ device enumeration supports **Logi Bolt** (USB PID `0xC548`) and
> **Logi Unifying** (PID `0xC52B` and others) receivers, as well as
> Bluetooth-direct devices — inherited from upstream OpenLogi.

## Prerequisites

- **Quit Solaar** (or any other Logitech manager) before starting Omalogi — the
  two applications fight over HID++ access.
- A kernel with `hidraw` and `uinput` module support (standard on Arch).
- `systemd` + `udev`.
- For the Hyprland actions: `hyprctl` and Omarchy's `omarchy-*` helpers on the
  agent's `PATH` (stock on Omarchy).

## Build from source

There are no pre-built Omalogi packages yet. Build with the stable Rust
toolchain:

```sh
git clone https://github.com/kurtlieber/omalogi
cd omalogi
cargo build --release -p openlogi -p openlogi-desktop -p openlogi-agent -p openlogi-overlay
```

The crates keep upstream's `openlogi-*` names so upstream merges stay clean
([ADR-0001](adr/0001-keep-crate-names.md)); the executables they build are
Omalogi's ([ADR-0006](adr/0006-omalogi-user-facing-identity.md)). Four production
executables land in `target/release/`:

| Binary | Role |
|---|---|
| `omalogi` | CLI — inventory, diagnostics, asset sync |
| `omalogi-desktop` | Desktop GUI |
| `omalogi-overlay` | Actions Ring overlay helper |
| `omalogi-agent` | Background agent — HID++ loop, input hook, Hyprland actions |

To build an Arch package instead (`.pkg.tar.zst`, plus `.deb`/`.rpm`):

```sh
cargo xtask linux package
sudo pacman -U target/release/omalogi-*.pkg.tar.zst
```

## NixOS

The repository Flake provides a package and a NixOS module for x86_64 and
aarch64 Linux:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.omalogi = {
    url = "github:kurtlieber/omalogi";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nixpkgs, omalogi, ... }: {
    nixosConfigurations.my-host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux"; # or aarch64-linux
      modules = [
        omalogi.nixosModules.default
        { programs.omalogi.enable = true; }
      ];
    };
  };
}
```

## Device access: udev rules

Omalogi needs:

- **Write access to `/dev/uinput`** — to create the virtual input device for
  button remapping.
- **Read/write access to `/dev/hidraw*`** — to send HID++ commands to the Bolt
  receiver, or to the device itself when it is paired over Bluetooth.
- **Read access to the mouse's `/dev/input/event*` node** — the hook grabs the
  pointer there to capture button presses. Bluetooth mice need the bundled rule
  for this: their event node hangs off `/devices/virtual/misc/uhid`, which has
  no seat, so `logind` never grants the ACL on its own.

Install the bundled udev rules to grant access to the active-seat user without
requiring `sudo` or group membership (requires `systemd-logind`):

```sh
sudo cp packaging/linux/udev/70-omalogi.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Verify access (should open without error):

```sh
# Check uinput
omalogi-agent --check-uinput 2>/dev/null || \
    test -w /dev/uinput && echo "uinput OK"

# Check a hidraw node
ls -la /dev/hidraw*

# Check the mouse's event node — look for a "+" (ACL) in the mode, or your
# user in the ACL itself. Without it the agent logs
# "could not install OS mouse hook".
getfacl /dev/input/event*
```

The GUI Settings → Permissions page shows a live `Granted` / `Not granted`
indicator; check it after installing the rules (no restart needed).

> **Device already connected?** `udevadm trigger` re-evaluates rules but does
> not re-grant `uaccess` ACLs on nodes that were already open when the rules
> were installed. If access is still denied, unplug and replug your receiver or
> mouse (or power-cycle for wireless devices) to let udev apply the new rules on
> reconnect.

### Non-systemd systems (SysV init, OpenRC)

Replace `TAG+="uaccess"` in the rules file with `MODE="0660", GROUP="input"`,
then add your user to the `input` group:

```sh
sudo usermod -aG input "$USER"
# Re-login for the group change to take effect.
```

## Install with the script

The `packaging/linux/install.sh` script copies the binaries, udev rules,
systemd unit, desktop entry, and icon to system paths, then reloads `udevadm`.

```sh
# From the repo root, after building:
sudo packaging/linux/install.sh
# Or to a custom prefix (e.g. /usr):
packaging/linux/install.sh --prefix=/usr
```

To remove:

```sh
packaging/linux/uninstall.sh
```

## Autostart (launch at login)

The background agent (`omalogi-agent`) must be running for the GUI and CLI to
show connected devices. Enable it for your user session:

```sh
systemctl --user enable --now omalogi-agent.service
```

Alternatively, toggle **Settings → General → Launch at login** in the GUI. When
a packaged unit is already installed it simply enables that one. Otherwise — a
build from source, or an install under a custom prefix — it generates a unit at
`~/.local/share/systemd/user/omalogi-agent.service` pointing at the running
binary.

Either way `~/.config/systemd/user/omalogi-agent.service` stays yours: systemd
ranks it above both locations, so a unit you write there overrides whatever
Omalogi does. Use `systemctl --user edit omalogi-agent.service` for a drop-in
that survives package upgrades.

Omalogi never overwrites or deletes a unit it did not generate, at either
location, and it only turns off an autostart it turned on. Enabling the service
yourself with the command above keeps working regardless of the GUI toggle.

## Verify the installation

```sh
# List connected Logitech devices:
omalogi list

# Launch the GUI:
omalogi-desktop
```

## Known limitations

| Limitation | Status |
|---|---|
| Per-application profiles on Hyprland | Keyed by the Wayland `app_id` (e.g. `org.mozilla.firefox`) via wlr-foreign-toplevel; XWayland apps by `WM_CLASS` |
| Button capture: middle / mode-shift / thumbwheel | Side buttons only today |
