# Huion HS64 Native Wayland Driver

A clean-room, memory-safe, native Wayland driver stack and GTK4/Libadwaita configuration utility for the **Huion HS64** graphic tablet (and compatible UC-Logic hardware), implemented entirely in Rust.

[![CI Tests](https://img.shields.io/badge/tests-passing-brightgreen.svg)]()
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)]()
[![Wayland Native](https://img.shields.io/badge/Wayland-Native-purple.svg)]()

---

## The Problem with the Official Driver

The official Huion Linux driver (`HuionTablet_LinuxDriver_v15.0.0.193.x86_64.deb`) was developed strictly for legacy X11 environments:
- **Hard X11 API Dependencies**: Binary analysis reveals hardcoded imports of `XOpenDisplay`, `XWarpPointer`, `XQueryPointer`, and `XRRGetMonitors`.
- **Foreign Process Key Injection**: Express keys are dispatched via bundled `xdotool` and `libxdo` (`XTestFakeKeyEvent`), which modern Wayland compositors reject for security.
- **Window Property Polling**: Active window detection relies on polling `_NET_ACTIVE_WINDOW` using `xprop`, which fails completely on native Wayland clients.
- **Root/Daemon Privilege Bloat**: The official driver relies on unconfined scripts and daemons.

Under modern Wayland compositors (GNOME Mutter, KDE Plasma KWin, Sway, Hyprland), the official driver fails to map tablet coordinates, ignores pressure curves, and fails to handle express keys.

See [research.md](research.md) for full binary disassembly, symbol analysis, and dependency breakdown.

---

## Architecture & Design

This project replaces the entire proprietary driver stack with a clean three-tier architecture:

```
                               ┌────────────────────────────────┐
                               │     Hardware: Huion HS64       │
                               │  (USB Vendor: 0x256c, HS64)    │
                               └───────────────┬────────────────┘
                                               │ Raw HID Reports
                                               ▼
                               ┌────────────────────────────────┐
                               │       /dev/hidrawX Node        │
                               └───────────────┬────────────────┘
                                               │
                        ┌──────────────────────┴───────────────────────┐
                        │                                               │
                        ▼                                               │
┌──────────────────────────────────────────────┐                       │
│             huion-daemon                     │                       │
│  ┌─────────────────────────────────────────┐ │                       │
│  │ 1. hidraw Reader & Packet Parser        │ │                       │
│  ├─────────────────────────────────────────┤ │                       │
│  │ 2. Coordinate Transformation Engine     │ │                       │
│  │    (Active area, aspect-ratio, rotation)│ │                       │
│  ├─────────────────────────────────────────┤ │                       │
│  │ 3. Cubic Bézier Pressure Curve Pipeline │ │                       │
│  ├─────────────────────────────────────────┤ │                       │
│  │ 4. Linux Kernel /dev/uinput Emitter     │ │                       │
│  │    - "Huion HS64 Wayland Stylus"        │ │                       │
│  │    - "Huion HS64 Wayland Pad"           │ │                       │
│  └────────────────────┬────────────────────┘ │                       │
│                       │                      │                       │
│  ┌────────────────────┴────────────────────┐ │                       │
│  │ 5. zbus D-Bus Service Provider          │ │                       │
│  │    org.wayland.HuionTablet              │ │                       │
│  └────────────────────▲────────────────────┘ │                       │
└───────────────────────┼──────────────────────┘                       │
                        │                                              │
                        │ D-Bus IPC (Method Calls & Signals)           │
                        │                                              ▼
┌───────────────────────┴──────────────────────┐        ┌──────────────────────────────┐
│              huion-gui                       │        │   Linux Kernel Input Subsys  │
│  ┌─────────────────────────────────────────┐ │        │   (/dev/input/event* nodes)  │
│  │ GTK4 / Libadwaita Wayland Client        │ │        └──────────────┬───────────────┘
│  │ - Dynamic Pressure Curve Editor         │ │                       │
│  │ - Orientation Selector (0/90/180/270)   │ │                       ▼
│  │ - Express Keys Action Mapper            │ │        ┌──────────────────────────────┐
│  │ - Interactive Pressure Scratchpad       │ │        │  Wayland Compositor          │
│  └─────────────────────────────────────────┘ │        │  (GNOME Mutter / KWin /      │
└──────────────────────────────────────────────┘        │   Sway / Hyprland)           │
                                                        └──────────────────────────────┘
```

### Crates Overview

1. **`crates/huion-proto`**:
   - Zero-allocation parser for Huion HS64 raw USB HID report descriptors:
     - Stylus report IDs `0x07`, `0x09`, `0x0A` (coordinates, 8192 pressure levels, barrel buttons, eraser, proximity).
     - Express keys report ID `0x08` (4 physical buttons bitmask).
   - Parametric cubic Bézier pressure curve engine with binary search inversion.
   - Coordinate rotation (0°, 90°, 180° left-handed mode, 270°) and active area mapping.
   - TOML configuration persistence (`$XDG_CONFIG_HOME/huion-wayland/config.toml`).

2. **`crates/huion-daemon`**:
   - Asynchronous sysfs discovery and hotplug polling for Huion devices (`VID: 0x256c`).
   - Virtual tablet generation via kernel `/dev/uinput` (`evdev` crate). Emits standard `EV_ABS` and `EV_KEY` events directly to the compositor.
   - Native DRM/KMS monitor enumeration (`/sys/class/drm/`) eliminating XRandR.
   - Session D-Bus service `org.wayland.HuionTablet` using `zbus 5.5`.

3. **`crates/huion-gui`**:
   - Modern GTK4 and Libadwaita graphical control panel.
   - Live Bézier pressure curve presets (Linear, Soft, Hard, Sigmoid) with interactive visual control.
   - Real-time pressure testing canvas (draw and inspect stroke dynamic width/opacity).
   - Express key assignment interface.
   - Async D-Bus client for live synchronization with `huion-daemon`.

---

## Hardware Specifications (Huion HS64)

- **Vendor ID**: `0x256c` (Huion / UC-Logic)
- **Product ID**: `0x006d` / `0x006b`
- **Active Area**: 160 × 102 mm (6.3 × 4.0 in)
- **Resolution**: 5080 LPI (Max Coordinates: `X = 40000`, `Y = 25000`)
- **Pen**: PW100 Battery-Free EMR Stylus
- **Pressure Levels**: 8192 levels (`0` to `8191`)
- **Express Keys**: 4 programmable physical buttons

---

## Installation & Setup

### Prerequisites

Ensure the following build dependencies and runtime packages are installed (Debian/Ubuntu/Fedora/Arch):

```bash
# Debian / Ubuntu / Pop!_OS
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libudev-dev

# Fedora
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel systemd-devel

# Arch Linux
sudo pacman -S base-devel pkgconf gtk4 libadwaita systemd-libs
```

### Building and Installing

Clone the repository and build via `make`:

```bash
git clone https://github.com/Nirusaki-Malaal/HuionHS64-Wayland.git
cd HuionHS64-Wayland

# Build release binaries and install system files
sudo make install
```

This installs:
- Binaries: `/usr/local/bin/huion-daemon` and `/usr/local/bin/huion-gui`
- Udev rule: `/etc/udev/rules.d/70-huion-wayland.rules` (grants user access to `/dev/uinput` and `/dev/hidraw*`)
- Systemd user service: `/usr/local/lib/systemd/user/huion-wayland.service`
- Desktop entry: `/usr/local/share/applications/io.github.nirusaki.HuionWayland.desktop`

### Permissions & Service Activation

1. **Reload udev rules** and plug in your tablet:
   ```bash
   sudo udevadm control --reload
   sudo udevadm trigger
   ```

2. **Enable and start the user service**:
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable --now huion-wayland.service
   ```

3. **Launch the settings GUI**:
   ```bash
   huion-gui
   ```
   Or open **Huion Tablet Settings** from your application launcher.

---

## Testing

Run the full automated test suite (including unit tests and end-to-end integration tests):

```bash
cargo test --workspace
```

The test suite covers:
- Raw packet parsing for stylus proximity, touch, barrel buttons, and eraser.
- Express key bitmask decoding and combos.
- Bézier curve calculations and pressure transformations.
- 180° rotation / left-handed mode coordinate inversion.
- Serialization and deserialization of tablet configuration profiles.

---

## Technical Documentation & Protocols

For deep technical details on the USB HID protocol, coordinate math formulas, D-Bus API methods, and security architecture, see:
- [Technical Documentation](documentation.md)
- [Reverse Engineering Findings](research.md)
- [Architecture & Implementation Plan](plan.md)

---

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
