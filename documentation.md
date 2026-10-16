# Implementation and operation

## Data flow and supported hardware

```text
HS64 physical USB (256c:006f), three hidraw interfaces
  -> nonblocking AsyncFd readers -> generation-tagged bounded channel
  -> parse standard pen/keyboard reports
  -> coordinates + configured pressure curve / configured shortcut state
  -> virtual stylus + virtual keyboard/mouse through uinput
  -> libinput -> compositor -> drawing application
```

Discovery selects one physical HS64 USB parent and all its raw interfaces. It
ignores other Huion products. Multiple physical tablets are not merged into one
input state. Interface paths are discovered, never fixed to hidraw7/8/9.

The event loop supervises discovery every second. All raw streams and virtual
devices must open successfully, and every physical evdev node for that USB parent
must be exclusively grabbed before any replacement events are emitted. If a
remapper owns a node, startup of that tablet session fails and retries without
injecting competing events. Session status becomes connected only after this
setup succeeds; detection alone is not a successful connection.

Async readers use nonblocking file descriptors and reactor readiness. Every
packet carries a session generation; packets queued before disconnect cannot
modify a new session. Disconnect, emission failure, SIGINT or SIGTERM tears down
the session. Virtual devices drop before physical grabs are released. No
blocking HID threads hold process shutdown open.

A locked file in `$XDG_RUNTIME_DIR/huion-wayland.lock` prevents a second daemon in
the same desktop session. D-Bus acquisition failure also terminates startup;
there is no duplicate-producing headless fallback. Cross-session contention is
ultimately resolved by exclusive evdev grabs. Run as the desktop user, not root.

## Standard-mode reports

The protocol details below were checked against connected-device HID
descriptors, captured reports and focused analysis of the vendor binaries.

### Pen: report 0x0a

| Bytes | Meaning |
| --- | --- |
| 0 | Report ID 0x0a |
| 1 bit 0 | Tip contact |
| 1 bits 1/2 | Barrel buttons |
| 1 bit 6 | In-range proximity |
| 2-3 | X, little endian, native maximum 32000 |
| 4-5 | Y, little endian, native maximum 20400 |
| 6-7 | Pressure, little endian |

The descriptor advertises pressure max 4095 but captured contact reports exceed
5400. Its pressure metadata is not a trustworthy scaling limit. Raw pressure is
retained within nominal 0-8191; no conversion from 4095 is applied. This does not
prove all 8192 values were exercised. A captured value above 4095 is a regression
fixture. X/Y are normalized into canonical 40000 × 25000 so existing saved active
areas remain compatible. Bit 4 is not proximity for this report. Hover and
proximity-out force pressure to zero.

The virtual pen uses BUS_VIRTUAL, no INPUT_PROP_DIRECT, and no fake tilt axes.
It advertises ABS_X, ABS_Y, ABS_PRESSURE and pen/touch/barrel keys. Its axis
resolutions describe the approximately 160 × 102 mm working surface. BTN_TOUCH
comes first in a frame; `evdev::VirtualDevice::emit` appends exactly one SYN_REPORT.
No relative pointer motion or synthetic X11 mouse events are generated.

Legacy parser layouts 0x07/0x09 remain covered by synthetic tests; discovery
claims support only for the observed HS64 standard-mode configuration.

### Buttons: report 0x03

The actual keyboard descriptor has a modifier byte followed directly by six
usage bytes, with no boot-keyboard reserved byte. Each known usage maps to a
physical pad button:

| Usage | Button |
| --- | --- |
| 0x1d (Z) | 1 |
| 0x2c (Space) | 2 |
| 0x30 (]) | 3 |
| 0x2f ([) | 4 |

All six slots are inspected. Unknown usages never become Button 4; an empty or
modifier-only report releases buttons. HID error/rollover reports preserve the
previous state until a valid report arrives. This usage-to-button association
was verified by captured reports and user confirmation on this host: with the
cable at the top, buttons1/2 are upper/lower left and3/4 upper/lower right.
Other firmware revisions still require confirmation. The physical shape follows
the [official HS64 manual](https://driverdl.huion.com/instruction/hs610-hs64/User_Manual_HS64.pdf),
page4 (printed3).

The shortcut engine reads `express_keys` for every report. It emits only state
changes, orders modifiers before key-down and after key-up, keeps shared
modifiers held until their final owner releases, and retriggers a held common
key when its modifiers change (Undo -> Redo). No simultaneous BTN_0..BTN_3
joystick events duplicate the keyboard action. Keyboard and mouse action
capabilities are advertised up front so configuration changes need no virtual
device recreation. Relative axes identify the shortcut device's mouse-button
capability; the daemon emits no relative motion.

### Vendor reports 0x08

Ghidra decompilation proves these multiplex pen, keys and other functions by
byte 1. Hardware keys use a different field at bytes 4-7; a second-byte four-bit
mask is incorrect. These packets remain Raw until vendor mode/capabilities are
implemented together. The daemon does not send mode-changing USB requests.
Vendor functions named `hid_get_feature_report` actually perform USB string
GET_DESCRIPTOR; copying the name as HIDIOCGFEATURE would be wrong.

## Configuration and IPC

Bus name and interface: `org.wayland.HuionTablet`.
Object path: `/org/wayland/HuionTablet`.

| Method | Return |
| --- | --- |
| GetConfig() | JSON configuration string |
| SetConfig(JSON string) | Boolean, true only after validated persistence |
| GetDeviceInfo() | JSON connected status and primary interface identity |
| GetDisplays() | JSON DRM connector/mode inventory |

Structural validation rejects zero/out-of-bounds active areas, nonfinite or
out-of-range curve control points, and malformed actions. The daemon also
checks actual evdev key names/capabilities. SetConfig rejects target_output or
aspect_ratio_locked because these features are not implemented. DRM mode
inventory does not establish logical compositor geometry or fractional scale.
Use your compositor's tablet output/aspect settings instead. The GUI explains
and clears legacy unsupported mapping flags when applying settings.

Disk and runtime updates share a write lock. Saving uses a unique sibling file,
write/sync, then atomic rename, so failure leaves the old configuration intact.
A failed disk save returns false and does not change live settings. Startup
validates the saved configuration; unsupported daemon settings fall back to
defaults with a log message.

## GUI

GTK4/Libadwaita >=1.2 provides separate Tablet, Buttons and Try your pen pages,
responsive settings groups, native action preset menus, custom shortcut editing
and an Apply action. A fixed cable-up tablet diagram shows the two left and two
right buttons with matching textual positions. The pressure page provides a
large expanding sheet; local0/90/180/270 rotation changes its portrait/landscape
frame and TOP guide, clears prior strokes, and preserves cursor/ink alignment.
It does not rotate system-wide tablet mapping. A worker owns D-Bus and disk
operations so GTK's main loop stays responsive. Connection polling does not
replace unsaved form edits. Apply rejection is displayed as an error; an offline
save is explicitly identified as local and requires daemon startup/restart.

GestureStylus provides compositor-delivered pressure to the meter and stroke
width. Mouse input never fabricates stylus pressure. The response-curve preview
is separate from this real input test. Soft presets increase output pressure at
a given input; firm presets decrease it. Custom curves are retained.

## Installation and coexistence

The project udev rules grant scoped HS64 hidraw/evdev access plus uinput access.
They clear the earlier LIBINPUT_IGNORE_DEVICE property on physical HS64 nodes.
Physical suppression is through live EVIOCGRAB handles, restoring kernel events
when the process exits. A compositor may need device replug/login to refresh
properties cached from an older install.

Remappers are another producer: on this machine keyd's wildcard owned the
physical HS64 keyboard. An explicit `-256c:006f` in its `[ids]` section excludes
both physical and replacement HS64 devices. See README for setup. Configuration
is backed up before local system edits; installation does not alter arbitrary
remapper configurations on other machines automatically.

The user service belongs to graphical-session.target and restarts on failure.
Autostart remains an explicit `systemctl --user enable` choice. Uninstall with
`sudo make uninstall`, stop/disable the user service, reload udev, and remove any
locally added remapper exclusion if desired. Keep your prior configuration
backup until hardware verification is satisfactory.

## Validation boundaries

Parser unit checks, synthetic kernel injection, live D-Bus/lifecycle checks and actual
drawing-app interaction are different levels of evidence. Passing one does not
establish all the others. No full-driver reconstruction, vendor-mode parity,
monitor mapping, tilt, or universal compositor compatibility is claimed.

## Supporting references

- [Linux uinput documentation](https://kernel.org/doc/html/latest/input/uinput.html)
  describes virtual devices and event emission.
- [Libinput tablet debugging](https://wayland.freedesktop.org/libinput/doc/latest/tablet-debugging.html)
  explains required tablet metadata and capability diagnosis.
- [Prachi Jha's tablet debugging account](https://dev.to/prachi_awesome_jha/how-i-fixed-my-drawing-tablet-by-patching-systemds-hardware-database-2d75)
  provides a relevant community example of tracing HID -> evdev -> libinput ->
  compositor instead of assuming USB detection proves usable tablet input. Its
  VEIKK-specific correction was not applied to this HS64; local evidence drove
  these fixes. No comments were present on the retrieved article.

## Kalam V1.0.0 release engineering

Kalam is a reverse engineering project made by nirusaki. Workspace metadata is
version1.0.0. GitHub Actions builds Linux x86_64 bundles on Ubuntu24.04, runs
locked tests, format/Clippy checks and an extracted staging installer regression,
and uploads the archive/checksum as workflow artifacts. Version tags call the
same checks before publishing assets through the release workflow.

`install.sh` delegates to the standard-library Python installer. Source builds
run without privilege; an extracted bundle uses its included binaries. File
installation escalates separately, writes atomically to allow upgrades of
running executables, refreshes udev, and enables boot loading of uinput. The
calling desktop user then starts/restarts the user service. `--enable` opts into
autostart. `--destdir` stages files with no sudo, module/udev operations, or
service changes. Uninstallation preserves personal configuration.

`tools/package_release.py` strips copies of release binaries, bundles scripts,
data, documentation and the GPL license, and writes SHA256SUMS plus
build provenance. Native GTK libraries are not bundled; the published build
requires Ubuntu24.04-compatible runtime libraries. See README.md for commands.

## Detailed installation and maintenance

The following covers manual binary/source installation, troubleshooting and tests.

## Easy installation

Download the Linux x86_64 build from [Kalam V1.0.0](https://github.com/Nirusaki-Malaal/HuionHS64-Wayland/releases/tag/v1.0.0).
Build files are attached to the release, and CI also uploads them as workflow
artifacts. Generated binaries and archives stay out of the Git source tree.

The prebuilt release is built on **Ubuntu 24.04 x86_64**, with glibc 2.39,
GTK4 4.14 and Libadwaita 1.5. Use Ubuntu 24.04 or newer, or a compatible current
Linux distribution. On older distributions, build from source instead. This is
a dynamically linked native GTK app, not a self-contained AppImage.

Install runtime dependencies on Ubuntu:

```sh
sudo apt-get update
sudo apt-get install -y libgtk-4-1 libadwaita-1-0 python3
```

Download, verify and install:

```sh
curl -fLO https://github.com/Nirusaki-Malaal/HuionHS64-Wayland/releases/download/v1.0.0/kalam-1.0.0-linux-x86_64.tar.gz
curl -fLO https://github.com/Nirusaki-Malaal/HuionHS64-Wayland/releases/download/v1.0.0/SHA256SUMS
sha256sum --check SHA256SUMS
tar -xzf kalam-1.0.0-linux-x86_64.tar.gz
cd kalam-1.0.0-linux-x86_64
./install.sh
huion-gui
```

Run the installer as your normal desktop user. It asks for sudo only to copy
system files, load uinput and refresh udev, then starts/restarts your user
service. It also configures uinput to load on boot. To enable service autostart,
use `./install.sh --enable`. To copy files without starting a service, use
`./install.sh --no-start`.

Uninstall from the extracted archive or source checkout:

```sh
./uninstall.sh
```

Uninstall stops/disables the user service and removes installed Kalam files.
Your personal tablet settings are preserved. It does not edit keyd settings.
Keep the extracted folder if you want to use its uninstall script later.

## Build from source

Requires Linux, a desktop session bus, Rust **1.89 or newer** (file locking),
Python **3.11 or newer**, GTK4, Libadwaita **1.2 or newer**, pkg-config, and uinput.
Install development dependencies on your distribution first:

```sh
# Ubuntu/Debian
sudo apt-get install -y build-essential libgtk-4-dev libadwaita-1-dev pkg-config python3
# Arch Linux
sudo pacman -S --needed base-devel rust gtk4 libadwaita python
```

Then install from the checkout with one command:

```sh
./install.sh
```

The source installer builds with `cargo build --release --locked --workspace`
as your normal user before escalating for system installation. If already
built, use `./install.sh --no-build`. For an isolated packaging test, use
`./install.sh --no-build --destdir /tmp/kalam-stage`; staging never changes
udev, modules, your service or the live system.

The service needs access to the physical tablet's hidraw **and evdev** nodes,
and `/dev/uinput`. Rules grant these through the active seat and `input` group.
Group changes require a new login. If another tablet driver owns the device,
stop it before starting Kalam.

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
that value. Pressure therefore passes through in the nominal 0-8191 range;
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
response are separate from synthetic kernel tests. The CI workflow checks code
and packaging; hardware behavior requires the local live-session checks above.

## CI/CD and release builds

[GitHub Actions](https://github.com/Nirusaki-Malaal/HuionHS64-Wayland/actions)
runs formatting, strict Clippy, locked workspace tests, script syntax checks,
metadata/no-em-dash checks, release compilation, and an extracted archive
install/upgrade/uninstall test. Every successful build uploads the binary
bundle and SHA256SUMS as an artifact.

A pushed `vX.Y.Z` tag reruns those checks and publishes a release named
`Kalam VX.Y.Z` with the checked archive and checksum. The tag must match the
Cargo workspace version. Only the publishing job has repository write access.
Hardware/kernel and live-session tests remain local because hosted runners
have no HS64 or desktop session.

To build the same archive locally:

```sh
cargo build --release --locked --workspace
python3 tools/package_release.py
python3 tests/packaging_e2e.py
```

The output is in `dist/`. `BUILD-INFO.json` inside the archive records its source
commit, Rust version, build distribution and native library versions. A local
build inherits your distribution's compatibility requirements; published builds
come from the Ubuntu CI runner.

## Repository layout

| Path | Purpose |
| --- | --- |
| `crates/huion-proto` | Report parsing, curves, rotation, validated atomic settings |
| `crates/huion-daemon` | HID streams, exclusive ownership, virtual input, D-Bus |
| `crates/huion-gui` | GTK/Adwaita settings and real pressure test |
| `tests` | Live D-Bus and service lifecycle checks |
| `tools/capture_tablet.py` | Bounded capture of HS64 interfaces only |


## Online installer

The README command builds from the current source for your own CPU architecture
and libc. It does not assume an Ubuntu binary can run on every Linux system.
First installation may take several minutes and needs internet access, free
build space, a working compiler toolchain and native GTK4/Libadwaita libraries.

The entry script is `bootstrap.sh`; platform logic is in
`scripts/online_install.py`. It checks Linux, rejects root execution, checks
Wayland and the desktop session bus, downloads a temporary source checkout,
installs build dependencies with sudo/doas, installs user-local Rust if needed,
compiles without root, and calls the regular file installer. The temporary
checkout is removed when it exits. It does not overwrite personal settings.

### Distribution support

| Distro family | Package manager |
| --- | --- |
| Debian, Ubuntu, Mint, Pop!_OS and derivatives | apt-get |
| Fedora, current RHEL derivatives | dnf |
| Arch, Manjaro, EndeavourOS, Artix | pacman |
| openSUSE | zypper |
| Alpine | apk |
| Void | xbps-install |
| Gentoo | emerge |

These are dependency mappings, not a claim that every version of every distro
has been hardware-tested. The live machine is Arch; hosted build/packaging tests
run on Ubuntu 24.04. Older releases without Libadwaita 1.2 or newer must be
upgraded or supplied newer development libraries. udev/eudev must be installed
and running. First download also needs curl and tar; bootstrap installs Python
if it is missing and a recognized package manager is present.

NixOS and immutable/OSTree systems are detected rather than changed with an
inappropriate package-manager command. Prepare the required native build
libraries and Rust in the host environment, then use `--no-deps`; a native Nix
package or immutable-system package is not supplied. An unknown distribution
can use an available supported package manager, or use prepared dependencies.
No generic script can guarantee support for every init system, compositor,
package repository or obsolete distribution.

### Installer options

```sh
# Show the detected distro and intended actions; no installation:
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/HuionHS64-Wayland/main/bootstrap.sh | sh -s -- --dry-run

# Use native dependencies and Rust already installed:
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/HuionHS64-Wayland/main/bootstrap.sh | sh -s -- --no-deps --enable

# Remove Kalam, keeping personal settings:
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/HuionHS64-Wayland/main/bootstrap.sh | sh -s -- --uninstall
```

`--enable` enables login startup. With a working systemd user manager it enables
the user service. Otherwise the installer starts a detached daemon and creates
an XDG autostart entry when requested. Your desktop must process XDG autostart
files; minimal compositors may need an explicit autostart command instead.
Standalone logs are in `$XDG_CACHE_HOME/kalam/daemon.log`, falling back to
`~/.cache/kalam/daemon.log`. `--no-start` skips starting or managing the daemon.

Autostart and standalone processes belong to your desktop user. System files
are installed with sudo or doas. The script never builds Rust as root and
never performs a full distribution upgrade. If keyd grabs the physical tablet,
apply the exclusion described in the installation section above.

## Resource measurements

The README table is measured from running installed processes, not estimated.
Raw data is in [docs/benchmarks.json](docs/benchmarks.json). Reproduce it with:

```sh
python3 tools/benchmark.py --seconds 30
```

Keep one driver and one settings window running and let the window settle before
starting. The tool samples `/proc/PID/smaps_rollup` and process CPU ticks once
per second for 30 seconds. It records process IDs, executable SHA-256 hashes,
CPU model, kernel, distribution, timestamp, memory medians, maximum observed RSS
and elapsed CPU usage. It does not synthesize input or restart processes.

PSS proportionally accounts for shared libraries; RSS includes all resident
shared mappings. CPU is averaged over the sample window, with 100% meaning one
fully occupied core. This is an idle observation with the GUI open, not a
latency, pen-load, startup-time or throughput benchmark. GUI rendering backends,
display drivers, window dimensions and normal interaction change the results.
The GUI may be closed while the driver keeps working. Measurements from this
Arch Wayland laptop are not guarantees for other computers.
