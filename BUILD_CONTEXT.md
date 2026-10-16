# Huion HS64 Wayland — current handoff

Updated 2026-10-16. This replaces the stale embedded-source bundle. The repository
files and Git history are the source of truth; do not reconstruct old source
from pasted excerpts.

## Goal and scope

Fix pressure handling, intermittent duplicate pointers, physical buttons and
settings UX for one connected standard-mode Huion HS64 (`256c:006f`). Preserve
native Wayland input through Linux uinput, with GTK4/Libadwaita frontend. No
vendor binary is executed or shipped. Binary decompilation is evidence only.

## Read first

- `README.md`: build/install/run, remapper coexistence, known scope.
- `documentation.md`: current protocols, ownership lifecycle, config and IPC.
- `research.md`: fresh findings precede the historical investigation.
- `research/vendor-analysis.md`: Ghidra functions, addresses, artifact hashes.
- `research/hardware-evidence.json`: descriptors, live pressure/button samples.
- `research/validation.md`: executed checks and remaining verification.

## Crucial findings

1. Native report0x0a X/Y max32000/20400 normalize into existing canonical
   40000/25000 settings. Proximity is bit6, not bit4.
2. Descriptor claims pressure max4095, but live values exceeded5400. Do not scale
   raw pressure from4095: that clips firm strokes. Keep nominal0–8191 raw values.
   Captured pressure4377 is covered by regression.
3. Vendor0x08 multiplexes pen/keys by byte1; it is not a four-key bitmask. Preserve
   Raw until vendor mode initialization and capability parsing are implemented
   together. Vendor getStringDesc ultimately sends USB GET_DESCRIPTOR STRING,
   not HIDIOCGFEATURE.
4. Keyboard03 layout is ID, modifiers, six usage bytes (no reserved byte).
   Known usages1d/2c/30/2f map to buttons1/2/3/4, with all captured on this host.
   Unknown usages and modifier-only reports must never invent Button4.
5. Old uinput code ignored saved bindings and emitted duplicate joystick codes.
   Configured actions now have edge-based ordered/shared modifier handling.
6. keyd wildcard grabbed the physical HS64 keyboard despite libinput ignore.
   Current host excludes `-256c:006f`; backup `/etc/keyd/default.conf.before-huion-wayland`.
7. Runtime evdev grabs, singleton locking and SIGTERM/disconnect cleanup replace
   permanent LIBINPUT_IGNORE_DEVICE. Failed ownership produces no replacement
   input. Physical fallback returns on stop. Old property is cleared by new rules.
8. Real GestureStylus pressure test replaces simulated mouse-pressure canvas.
   Native presets/custom shortcuts and honest status/saving replace static labels.

## Source ownership

`huion-proto`: parser, canonical units, curve, rotation, structural validation,
atomic configuration save. `huion-daemon`: AsyncFd streams, generation-tagged
packet supervision, uinput frame/shortcut state, singleton, evdev grabs, IPC.
`huion-gui`: native settings, presets, custom validation, real pressure strokes,
background D-Bus and connection status.

## Validation and operating state

Use `research/validation.md` for final observed results. Normal commands:

```sh
cargo test --workspace
cargo test -p huion-daemon -- --include-ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --release --workspace
python tests/session_e2e.py
python tests/lifecycle_e2e.py
```

Host quirk: cargo clippy/fmt hit unconfigured rustup shims. Existing system
binaries work as `/usr/bin/cargo-clippy clippy ...` and `/usr/bin/cargo-fmt fmt ...`.
Do not install toolchains or rewrite global configuration to work around that.

Install already-built release binaries with `sudo make -o build install`; build
first as normal user. User service `huion-wayland.service` is running but was not
enabled for autostart. Local system artifacts have `.before-20261016-fixes` backups.

Live D-Bus: org.wayland.HuionTablet at /org/wayland/HuionTablet. GetConfig,
SetConfig(string JSON)->bool, GetDeviceInfo, GetDisplays. SetConfig returns false
on invalid keys/areas, unsupported output/aspect mapping or persistence failure.

## Boundaries

No native vendor0x08 mode, tilt, multiple simultaneous tablets or compositor
output/aspect integration. GUI explains compositor mapping and can clear stale
unsupported settings. DRM inventory is not compositor logical geometry.
The user confirmed pressure/buttons/pointer behavior works and verified physical
button order (upper/lower left =1/2; upper/lower right =3/4, cable up). Synthetic
tests and that host-specific confirmation do not prove every drawing app or firmware.
The follow-up UI request adds an enlarged separate pen workspace with local sheet
rotation and an accurate visual/text button map; see latest GUI source/tests.

REA MCP tools were unavailable; read-only doctor reported missing registration.
Existing /opt/ghidra supplied actual decompilation without modifying MCP config.
No restart/setup approval was needed for that fallback.

Licensing has an inherited inconsistency: Cargo says GPL-3.0-or-later while the
repository ships MIT/Apache texts. Preserve owner choice; resolve before release.
