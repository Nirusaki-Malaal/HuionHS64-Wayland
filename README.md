# Huion HS64 on Wayland

A Rust userspace driver and native GTK4/Libadwaita settings app for the connected
Huion HS64 (`256c:006f`). The daemon reads HID reports and creates a Linux tablet
and shortcut device through `/dev/uinput`; it does not use X11 or XTest injection.

![Settings interface](docs/images/settings.png)

[Large pen workspace](docs/images/pen-workspace.png) ·
[Physical button layout](docs/images/buttons.png)

## Current behavior

- Standard-mode pen coordinates, pressure, hover, tip and two barrel buttons.
- Pressure response curves, tablet active-area configuration and four rotations.
- Four configurable express buttons with shared-modifier handling and releases.
- Native settings interface with action presets, editable custom shortcuts,
  actual stylus pressure/stroke testing, and live driver status.
- One daemon per user session; exclusive physical input ownership prevents
  kernel fallback events from competing with virtual input.
- SIGTERM/disconnect cleanup releases virtual devices and physical grabs.

This implementation targets the observed HS64 standard mode (`0x0a` pen,
`0x03` keyboard). Vendor `0x08` reports are preserved but not interpreted or
used to switch firmware modes. Other Huion models are not supported. Display
selection and aspect-ratio mapping belong to the compositor; tilt is not
advertised by this HS64. See [research.md](research.md) for the evidence and
[documentation.md](documentation.md) for implementation details and limitations.

## Build and install

Requires Linux, a desktop session bus, Rust **1.89 or newer** (file locking),
GTK4, Libadwaita **1.2 or newer**, pkg-config, and the uinput kernel module.
On Arch, the native development dependencies are `base-devel gtk4 libadwaita`.

```sh
cargo build --release --workspace
cargo test --workspace
sudo make -o build install
sudo udevadm control --reload
sudo udevadm trigger --subsystem-match=input
sudo udevadm trigger --subsystem-match=hidraw
systemctl --user daemon-reload
systemctl --user start huion-wayland.service
huion-gui
```

Build as your normal user first; `-o build` prevents the privileged install from
rebuilding through root's Cargo environment. Installation does not enable
autostart. To enable it explicitly:

```sh
systemctl --user enable huion-wayland.service
```

The service needs access to the physical tablet's hidraw **and evdev** nodes,
and `/dev/uinput`. Rules grant these through the active seat and `input` group.
Group changes require a new login. If another tablet driver owns the device,
stop it before starting this daemon.

### If you use keyd or another input remapper

A wildcard keyd configuration can grab the physical HS64 keyboard and re-inject
its factory shortcuts. Libinput ignore rules do not block that path. Exclude
this tablet's IDs in the matching keyd configuration's `[ids]` section:

```ini
[ids]
*
-256c:006f
```

Then run `sudo keyd reload`. Back up your configuration before editing. Both
physical and replacement devices use these IDs; all other keyboard mappings
can remain in place. The daemon refuses to inject tablet input when it cannot
obtain exclusive ownership. Look for `Cannot exclusively grab` in its log.

## Configure and verify

Open `huion-gui`. Use **Tablet** for pressure/orientation and **Buttons** to
choose named actions beside the physical layout diagram, then click **Apply**.
With the cable at the top, buttons1/2 are the left pair and3/4 the right pair.

Open the separate **Try your pen** page for a larger expanding workspace. Its
rotation control turns the test sheet between landscape and portrait while
keeping strokes under the cursor; it does not change system tablet mapping.
Draw in **Try your pen**: its meter and stroke width use actual
stylus pressure delivered by the compositor. Mouse motion does not simulate
pressure. Also test in your drawing app with a brush that uses pen pressure.

Settings live in `$XDG_CONFIG_HOME/huion-wayland/config.toml`, falling back to
`~/.config/huion-wayland/config.toml`. Offline saves take effect on daemon
restart. Live saves apply through D-Bus and are persisted atomically.

```sh
journalctl --user -u huion-wayland.service -f
python tools/capture_tablet.py --seconds 30 --output /tmp/hs64.json
```

The descriptor declares pressure max 4095, but live standard-mode reports exceed
that value. Pressure therefore passes through in the nominal 0–8191 range;
scaling from the descriptor would clip firm strokes. Coordinates are normalized
from native 32000 × 20400 into the existing configuration range 40000 × 25000.

## Tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
# Requires uinput/evdev access, with the HS64 excluded from remappers:
cargo test -p huion-daemon -- --include-ignored
# Requires the installed running service (roundtrips existing settings):
python tests/session_e2e.py
# Briefly stops/restarts the service and checks ownership/fallback:
python tests/lifecycle_e2e.py
```

Kernel tests grab their temporary devices before sending test events. They fail
if a remapper owns those devices. Hardware interaction and application pressure
response are separate from synthetic kernel tests. Validation results and
remaining checks are recorded in [research/validation.md](research/validation.md).

## Repository layout

| Path | Purpose |
| --- | --- |
| `crates/huion-proto` | Report parsing, curves, rotation, validated atomic settings |
| `crates/huion-daemon` | HID streams, exclusive ownership, virtual input, D-Bus |
| `crates/huion-gui` | GTK/Adwaita settings and real pressure test |
| `research/vendor-analysis.md` | Ghidra/objdump findings, hashes and addresses |
| `tests` | Live D-Bus and service lifecycle checks |
| `tools/capture_tablet.py` | Bounded capture of HS64 interfaces only |

## License

The existing repository ships [MIT](LICENSE-MIT) and
[Apache 2.0](LICENSE-APACHE) license texts. Cargo metadata currently declares
GPL-3.0-or-later; this preexisting inconsistency needs an owner decision before
a release. The supplied proprietary Huion package and decompilation dumps are
not redistributed in this repository.
