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
