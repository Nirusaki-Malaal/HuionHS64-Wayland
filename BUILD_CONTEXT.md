# Huion HS64 Native Wayland Driver — Full Build Context & Architecture

This document aggregates the entire codebase, protocol reverse-engineering findings, hardware specifications, and system integration configuration for the native Wayland driver stack of the **Huion HS64** graphic tablet.

---

## 1. System Overview & Problem Statement

### 1.1 Why Official Driver Fails Under Wayland
The official Huion Linux driver (`HuionTablet_LinuxDriver_v15.0.0.193.x86_64.deb`) relies on legacy X11 APIs:
1. Hardcoded dynamic imports of `libX11` and `libXrandr` (`XOpenDisplay`, `XWarpPointer`, `XQueryPointer`, `XRRGetMonitors`). Under native Wayland compositors (Mutter, KWin, Sway, Hyprland), global pointer warping and querying are denied.
2. Active window polling via `popen("xprop -root _NET_ACTIVE_WINDOW")`. Returns empty strings on Wayland.
3. Express key injection using bundled `xdotool` and `libxdo` (`XTestFakeKeyEvent`). Ignored by native Wayland applications.
4. Duplicate pointers: When `hid-generic` attaches to the USB tablet, kernel `mouse` and `kbd` nodes emit raw uncalibrated events simultaneously with virtual tablet drivers unless inhibited via udev.

### 1.2 Clean-Room Rust Solution
- **`crates/huion-proto`**: Pure zero-allocation HID packet parser, parametric cubic Bézier pressure curve calculator (binary search inversion), coordinate rotation (0°, 90°, 180°, 270°), active area clipping, and TOML configuration serialization.
- **`crates/huion-daemon`**: Asynchronous background service discovering `/dev/hidraw*` composite interfaces, reading raw reports, converting them to standard Linux `/dev/uinput` absolute stylus and keyboard/pad events via `evdev`, enumerating monitors via DRM/KMS sysfs (`/sys/class/drm`), and publishing D-Bus interface `org.wayland.HuionTablet`.
- **`crates/huion-gui`**: Native GTK4 / Libadwaita control panel with live pressure testing scratchpad, Bézier curve profile selector, orientation toggle, and asynchronous D-Bus sync.
- **`data/70-huion-wayland.rules`**: Udev rule setting `MODE="0660"`, `GROUP="input"`, `TAG+="uaccess"`, and `ENV{LIBINPUT_IGNORE_DEVICE}="1"` on the raw USB nodes to eliminate dual pointer / key leakage.

---

## 2. Hardware Specification (Huion HS64)

- **USB Vendor ID**: `0x256c`
- **USB Product ID**: `0x006f` (also matches `0x006b`, `0x006d`)
- **Max Absolute Coordinates**: `X = 40000`, `Y = 25000` (5080 LPI resolution, 160 × 102 mm active area)
- **Stylus**: PW100 Battery-Free EMR Pen (8192 pressure levels, `0` to `8191`)
- **Composite USB Interfaces**:
  - `Interface 0` (`/dev/hidraw7`): Vendor/Pad endpoints (Report ID `0x08`)
  - `Interface 1` (`/dev/hidraw8`): Digitizer Pen endpoint (Report ID `0x0a` / `0x07` / `0x09`)
  - `Interface 2` (`/dev/hidraw9`): HID Keyboard endpoint (Report ID `0x03`) emitting hardware shortcut scancodes

---

## 3. Reverse-Engineered HID Protocol

### 3.1 Stylus Report (`0x0a` / `0x07` / `0x09` on Interface 1)
Packet size: 8 to 12 bytes. Little-endian numbers.

```
Byte 0: Report ID (0x0a, 0x07, or 0x09)
Byte 1: Status Bitmask
        - Bit 0 (0x01): Tip Touch (pen contact with surface)
        - Bit 1 (0x02): Barrel Button 1 (lower stylus button)
        - Bit 2 (0x04): Barrel Button 2 / Eraser (upper stylus button)
        - Bit 3 (0x08): Invert
        - Bit 4 (0x10): In-Range Proximity (legacy mode)
        - Bit 6 (0x40): In-Range Proximity (standard HS64 mode, 0xc0 during hover)
Bytes 2..3: Absolute X Coordinate (u16 LE, 0 .. 40000)
Bytes 4..5: Absolute Y Coordinate (u16 LE, 0 .. 25000)
Bytes 6..7: Pressure Level (u16 LE, 0 .. 8191)
Bytes 10..11: Signed Tilt X and Tilt Y (-60 .. +60) [Optional]
```

### 3.2 Express Keys Report (`0x03` on Interface 2 / `0x08` on Interface 0)
On HS64 (`0x006f`), the physical buttons stream standard HID keyboard reports on Report ID `0x03`:

```
Byte 0: Report ID (0x03)
Byte 1: Modifier bitmask (0x05 = Ctrl + Alt)
Byte 2: Keycode:
        - 0x1d: Button 1 (Top) -> Hardware sends Ctrl+Alt+Z (Undo)
        - 0x2c: Button 2 -> Hardware sends Space (Hand/Pan)
        - 0x30: Button 3 -> Hardware sends ']' (Brush size increase)
        - 0x2f: Button 4 (Bottom) -> Hardware sends '[' (Brush size decrease)
        - 0x00: All buttons released
```

---

## 4. File Tree & Build Layout

```
.
├── Cargo.lock
├── Cargo.toml
├── Makefile
├── README.md
├── documentation.md
├── plan.md
├── research.md
├── data
│   ├── 70-huion-wayland.rules
│   ├── huion-wayland.service
│   └── io.github.nirusaki.HuionWayland.desktop
└── crates
    ├── huion-proto
    │   ├── Cargo.toml
    │   ├── src
    │   │   ├── bezier.rs
    │   │   ├── config.rs
    │   │   ├── lib.rs
    │   │   └── packet.rs
    │   └── tests
    │       └── e2e_test.rs
    ├── huion-daemon
    │   ├── Cargo.toml
    │   └── src
    │       ├── dbus.rs
    │       ├── display.rs
    │       ├── hidraw.rs
    │       ├── main.rs
    │       └── uinput.rs
    └── huion-gui
        ├── Cargo.toml
        └── src
            └── main.rs
```

---

## 5. System Configuration Files

### `data/70-huion-wayland.rules`
```udev
# Udev rules for Huion HS64 Native Wayland Driver
# Grants user access to Huion USB devices and uinput

# Allow input group and logged-in user to access Huion USB tablets
KERNEL=="hidraw*", ATTRS{idVendor}=="256c", MODE="0660", GROUP="input", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="256c", MODE="0660", GROUP="input", TAG+="uaccess"

# Allow user access to /dev/uinput for virtual tablet creation
KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="input", TAG+="uaccess"

# Tell libinput to ignore the raw physical USB tablet and keyboard interfaces
# This prevents:
# 1. Two-pointer conflict (raw kernel mouse5/event23 fighting virtual stylus)
# 2. Raw factory scancodes leakage (raw kernel event24 fighting custom express key bindings)
ACTION=="add|change", SUBSYSTEM=="input", ATTRS{idVendor}=="256c", ATTRS{idProduct}=="006f", ENV{LIBINPUT_IGNORE_DEVICE}="1"
```

### `data/huion-wayland.service`
```ini
[Unit]
Description=Huion HS64 Native Wayland Tablet Daemon
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/huion-daemon
Restart=always
RestartSec=2
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=default.target
```

### `Makefile`
```makefile
PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
UDEVDIR ?= /etc/udev/rules.d
SYSTEMDUSERDIR ?= $(PREFIX)/lib/systemd/user
APPDIR ?= $(PREFIX)/share/applications

.PHONY: all build test install uninstall clean

all: build

build:
	cargo build --release

test:
	cargo test --workspace

install: build
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 target/release/huion-daemon $(DESTDIR)$(BINDIR)/huion-daemon
	install -m 755 target/release/huion-gui $(DESTDIR)$(BINDIR)/huion-gui
	install -d $(DESTDIR)$(UDEVDIR)
	install -m 644 data/70-huion-wayland.rules $(DESTDIR)$(UDEVDIR)/70-huion-wayland.rules
	install -d $(DESTDIR)$(SYSTEMDUSERDIR)
	install -m 644 data/huion-wayland.service $(DESTDIR)$(SYSTEMDUSERDIR)/huion-wayland.service
	install -d $(DESTDIR)$(APPDIR)
	install -m 644 data/io.github.nirusaki.HuionWayland.desktop $(DESTDIR)$(APPDIR)/io.github.nirusaki.HuionWayland.desktop
	@echo "Installation complete. Reload udev rules using: sudo udevadm control --reload && sudo udevadm trigger"

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/huion-daemon
	rm -f $(DESTDIR)$(BINDIR)/huion-gui
	rm -f $(DESTDIR)$(UDEVDIR)/70-huion-wayland.rules
	rm -f $(DESTDIR)$(SYSTEMDUSERDIR)/huion-wayland.service
	rm -f $(DESTDIR)$(APPDIR)/io.github.nirusaki.HuionWayland.desktop

clean:
	cargo clean
```

---

## 6. Rust Source Manifests & Implementations

### Root `Cargo.toml`
```toml
[workspace]
resolver = "2"
members = [
    "crates/huion-proto",
    "crates/huion-daemon",
    "crates/huion-gui",
]
```

### `crates/huion-proto/Cargo.toml`
```toml
[package]
name = "huion-proto"
version = "0.1.0"
edition = "2021"
description = "Pure parser, Bezier math, and configuration for Huion HS64"
license = "MIT OR Apache-2.0"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
thiserror = "1.0"
```

### `crates/huion-proto/src/packet.rs`
```rust
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ProtocolError {
    #[error("packet too short: expected at least {expected} bytes, got {actual}")]
    PacketTooShort { expected: usize, actual: usize },
    #[error("unsupported report id: 0x{0:02x}")]
    UnsupportedReportId(u8),
    #[error("invalid coordinate data")]
    InvalidCoordinate,
}

pub const HS64_MAX_X: u16 = 40000;
pub const HS64_MAX_Y: u16 = 25000;
pub const HS64_MAX_PRESSURE: u16 = 8191;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StylusPacket {
    pub x: u16,
    pub y: u16,
    pub pressure: u16,
    pub touch: bool,
    pub barrel_switch: bool,
    pub eraser: bool,
    pub in_range: bool,
    pub tilt_x: i8,
    pub tilt_y: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ExpressKeyPacket {
    pub raw_mask: u8,
    pub key1: bool,
    pub key2: bool,
    pub key3: bool,
    pub key4: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TabletReport {
    Stylus(StylusPacket),
    ExpressKeys(ExpressKeyPacket),
    Raw(Vec<u8>),
}

pub fn parse_report(data: &[u8]) -> Result<Option<TabletReport>, ProtocolError> {
    if data.is_empty() {
        return Ok(None);
    }

    let report_id = data[0];
    match report_id {
        0x07 | 0x09 | 0x0a => {
            if data.len() < 8 {
                return Err(ProtocolError::PacketTooShort {
                    expected: 8,
                    actual: data.len(),
                });
            }

            let status = data[1];
            let touch = (status & 0x01) != 0;
            let barrel_switch = (status & 0x02) != 0;
            let eraser = (status & 0x04) != 0;
            let in_range = (status & 0x40) != 0 || (status & 0x10) != 0 || touch || barrel_switch || eraser;

            let x = u16::from_le_bytes([data[2], data[3]]);
            let y = u16::from_le_bytes([data[4], data[5]]);
            let pressure = u16::from_le_bytes([data[6], data[7]]);

            let (tilt_x, tilt_y) = if data.len() >= 12 {
                (data[10] as i8, data[11] as i8)
            } else {
                (0, 0)
            };

            Ok(Some(TabletReport::Stylus(StylusPacket {
                x,
                y,
                pressure: pressure.min(HS64_MAX_PRESSURE),
                touch,
                barrel_switch,
                eraser,
                in_range,
                tilt_x,
                tilt_y,
            })))
        }

        0x08 => {
            if data.len() < 2 {
                return Err(ProtocolError::PacketTooShort {
                    expected: 2,
                    actual: data.len(),
                });
            }

            let mask = data[1];
            Ok(Some(TabletReport::ExpressKeys(ExpressKeyPacket {
                raw_mask: mask,
                key1: (mask & 0x01) != 0,
                key2: (mask & 0x02) != 0,
                key3: (mask & 0x04) != 0,
                key4: (mask & 0x08) != 0,
            })))
        }

        0x03 => {
            if data.len() < 3 {
                return Err(ProtocolError::PacketTooShort {
                    expected: 3,
                    actual: data.len(),
                });
            }

            let modifier = data[1];
            let keycode = data[2];

            if modifier == 0 && keycode == 0 {
                return Ok(Some(TabletReport::ExpressKeys(ExpressKeyPacket {
                    raw_mask: 0,
                    key1: false,
                    key2: false,
                    key3: false,
                    key4: false,
                })));
            }

            let (k1, k2, k3, k4) = match keycode {
                0x1d => (true, false, false, false),
                0x2c => (false, true, false, false),
                0x30 => (false, false, true, false),
                _ => (false, false, false, true),
            };

            let mask = (if k1 { 1 } else { 0 })
                | (if k2 { 2 } else { 0 })
                | (if k3 { 4 } else { 0 })
                | (if k4 { 8 } else { 0 });

            Ok(Some(TabletReport::ExpressKeys(ExpressKeyPacket {
                raw_mask: mask,
                key1: k1,
                key2: k2,
                key3: k3,
                key4: k4,
            })))
        }

        0xf7 | 0xf8 | 0xf9 => Ok(Some(TabletReport::Raw(data.to_vec()))),
        _ => Err(ProtocolError::UnsupportedReportId(report_id)),
    }
}
```

### `crates/huion-proto/src/bezier.rs`
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BezierPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PressureCurve {
    pub p1: BezierPoint,
    pub p2: BezierPoint,
}

impl Default for PressureCurve {
    fn default() -> Self {
        Self {
            p1: BezierPoint { x: 0.0, y: 0.0 },
            p2: BezierPoint { x: 1.0, y: 1.0 },
        }
    }
}

impl PressureCurve {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self {
            p1: BezierPoint {
                x: x1.clamp(0.0, 1.0),
                y: y1.clamp(0.0, 1.0),
            },
            p2: BezierPoint {
                x: x2.clamp(0.0, 1.0),
                y: y2.clamp(0.0, 1.0),
            },
        }
    }

    pub fn evaluate(&self, normalized_pressure: f32) -> f32 {
        let x_target = normalized_pressure.clamp(0.0, 1.0);
        let mut t_low = 0.0f32;
        let mut t_high = 1.0f32;
        let mut t = x_target;

        for _ in 0..16 {
            let one_minus_t = 1.0 - t;
            let current_x = 3.0 * one_minus_t * one_minus_t * t * self.p1.x
                + 3.0 * one_minus_t * t * t * self.p2.x
                + t * t * t;

            if (current_x - x_target).abs() < 1e-4 {
                break;
            }

            if current_x < x_target {
                t_low = t;
            } else {
                t_high = t;
            }
            t = (t_low + t_high) * 0.5;
        }

        let one_minus_t = 1.0 - t;
        let y = 3.0 * one_minus_t * one_minus_t * t * self.p1.y
            + 3.0 * one_minus_t * t * t * self.p2.y
            + t * t * t;

        y.clamp(0.0, 1.0)
    }

    pub fn map_pressure(&self, raw_pressure: u16, max_pressure: u16) -> u16 {
        if max_pressure == 0 {
            return 0;
        }
        let normalized = raw_pressure as f32 / max_pressure as f32;
        let mapped = self.evaluate(normalized);
        (mapped * max_pressure as f32).round() as u16
    }
}
```

### `crates/huion-proto/src/config.rs`
```rust
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

use crate::bezier::PressureCurve;
use crate::packet::{HS64_MAX_X, HS64_MAX_Y};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveArea {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Default for ActiveArea {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: HS64_MAX_X,
            height: HS64_MAX_Y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabletConfig {
    pub device_name: String,
    pub orientation: Orientation,
    pub active_area: ActiveArea,
    pub pressure_curve: PressureCurve,
    pub monitor_output: Option<String>,
}

impl Default for TabletConfig {
    fn default() -> Self {
        Self {
            device_name: "Huion HS64".to_string(),
            orientation: Orientation::default(),
            active_area: ActiveArea::default(),
            pressure_curve: PressureCurve::default(),
            monitor_output: None,
        }
    }
}

impl TabletConfig {
    pub fn config_path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            });
        base.join("huion-wayland").join("config.toml")
    }

    pub fn load_or_default() -> Self {
        let p = Self::config_path();
        if p.exists() {
            if let Ok(content) = fs::read_to_string(&p) {
                if let Ok(cfg) = toml::from_str(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let p = Self::config_path();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        fs::write(p, content)
    }

    pub fn transform_coordinates(&self, raw_x: u16, raw_y: u16) -> (u16, u16) {
        let (cx, cy) = (
            raw_x.clamp(self.active_area.x, self.active_area.x + self.active_area.width),
            raw_y.clamp(self.active_area.y, self.active_area.y + self.active_area.height),
        );

        let norm_x = ((cx - self.active_area.x) as u32 * HS64_MAX_X as u32
            / self.active_area.width as u32) as u16;
        let norm_y = ((cy - self.active_area.y) as u32 * HS64_MAX_Y as u32
            / self.active_area.height as u32) as u16;

        match self.orientation {
            Orientation::Normal => (norm_x, norm_y),
            Orientation::Rotate180 => (HS64_MAX_X.saturating_sub(norm_x), HS64_MAX_Y.saturating_sub(norm_y)),
            Orientation::Rotate90 => (HS64_MAX_X.saturating_sub(norm_y), norm_x),
            Orientation::Rotate270 => (norm_y, HS64_MAX_Y.saturating_sub(norm_x)),
        }
    }
}
```

### `crates/huion-daemon/Cargo.toml`
```toml
[package]
name = "huion-daemon"
version = "0.1.0"
edition = "2021"
description = "Native Wayland input daemon, uinput emitter, and D-Bus service"
license = "MIT OR Apache-2.0"

[dependencies]
huion-proto = { path = "../huion-proto" }
tokio = { version = "1.0", features = ["full"] }
evdev = { version = "0.12", features = ["tokio"] }
zbus = "5.5"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
```

### `crates/huion-daemon/src/uinput.rs`
```rust
use std::io;
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{
    AbsoluteAxisType, AttributeSet, BusType, EventType, InputEvent,
    InputId, Key, PropType, UinputAbsSetup,
};
use huion_proto::config::TabletConfig;
use huion_proto::packet::{ExpressKeyPacket, StylusPacket, HS64_MAX_PRESSURE, HS64_MAX_X, HS64_MAX_Y};

pub struct VirtualTablet {
    pen_device: VirtualDevice,
    pad_device: VirtualDevice,
}

impl VirtualTablet {
    pub fn new() -> io::Result<Self> {
        let abs_x = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_X,
            evdev::AbsInfo::new(0, 0, HS64_MAX_X as i32, 0, 0, 200),
        );
        let abs_y = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_Y,
            evdev::AbsInfo::new(0, 0, HS64_MAX_Y as i32, 0, 0, 200),
        );
        let abs_pressure = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_PRESSURE,
            evdev::AbsInfo::new(0, 0, HS64_MAX_PRESSURE as i32, 0, 0, 0),
        );
        let abs_tilt_x = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_TILT_X,
            evdev::AbsInfo::new(0, -60, 60, 0, 0, 0),
        );
        let abs_tilt_y = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_TILT_Y,
            evdev::AbsInfo::new(0, -60, 60, 0, 0, 0),
        );

        let mut pen_keys = AttributeSet::<Key>::new();
        pen_keys.insert(Key::BTN_TOOL_PEN);
        pen_keys.insert(Key::BTN_TOUCH);
        pen_keys.insert(Key::BTN_STYLUS);
        pen_keys.insert(Key::BTN_STYLUS2);

        let mut pen_props = AttributeSet::<PropType>::new();
        pen_props.insert(PropType::DIRECT);

        let pen_device = VirtualDeviceBuilder::new()?
            .name("Huion HS64 Wayland Stylus")
            .input_id(InputId::new(BusType::BUS_USB, 0x256c, 0x006d, 1))
            .with_keys(&pen_keys)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .with_absolute_axis(&abs_pressure)?
            .with_absolute_axis(&abs_tilt_x)?
            .with_absolute_axis(&abs_tilt_y)?
            .with_properties(&pen_props)?
            .build()?;

        let mut pad_keys = AttributeSet::<Key>::new();
        pad_keys.insert(Key::BTN_0);
        pad_keys.insert(Key::BTN_1);
        pad_keys.insert(Key::BTN_2);
        pad_keys.insert(Key::BTN_3);
        pad_keys.insert(Key::KEY_LEFTCTRL);
        pad_keys.insert(Key::KEY_LEFTSHIFT);
        pad_keys.insert(Key::KEY_Z);
        pad_keys.insert(Key::KEY_B);
        pad_keys.insert(Key::KEY_E);

        let pad_device = VirtualDeviceBuilder::new()?
            .name("Huion HS64 Wayland Pad")
            .input_id(InputId::new(BusType::BUS_USB, 0x256c, 0x006d, 1))
            .with_keys(&pad_keys)?
            .build()?;

        Ok(Self {
            pen_device,
            pad_device,
        })
    }

    pub fn emit_stylus(&mut self, packet: StylusPacket, config: &TabletConfig) -> io::Result<()> {
        let (x, y) = config.transform_coordinates(packet.x, packet.y);
        let pressure = config.pressure_curve.map_pressure(packet.pressure, HS64_MAX_PRESSURE);

        let events = [
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, x as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, y as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_PRESSURE.0, pressure as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_TILT_X.0, packet.tilt_x as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_TILT_Y.0, packet.tilt_y as i32),
            InputEvent::new(EventType::KEY, Key::BTN_TOOL_PEN.code(), if packet.in_range { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_TOUCH.code(), if packet.touch { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_STYLUS.code(), if packet.barrel_switch { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_STYLUS2.code(), if packet.eraser { 1 } else { 0 }),
            InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
        ];

        self.pen_device.emit(&events)
    }

    pub fn emit_express_keys(&mut self, packet: ExpressKeyPacket) -> io::Result<()> {
        let events = [
            InputEvent::new(EventType::KEY, Key::BTN_0.code(), if packet.key1 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_1.code(), if packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_2.code(), if packet.key3 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_3.code(), if packet.key4 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), if packet.key1 || packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_LEFTSHIFT.code(), if packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_Z.code(), if packet.key1 || packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_B.code(), if packet.key3 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_E.code(), if packet.key4 { 1 } else { 0 }),
            InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
        ];

        self.pad_device.emit(&events)
    }
}
```

### `crates/huion-daemon/src/main.rs`
```rust
mod dbus;
mod display;
mod hidraw;
mod uinput;

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex, RwLock};
use tracing::{error, info, warn};

use huion_proto::config::TabletConfig;
use huion_proto::packet::{parse_report, TabletReport};

use crate::dbus::HuionDbusService;
use crate::hidraw::{find_huion_devices, DeviceInfo};
use crate::uinput::VirtualTablet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Huion HS64 Native Wayland Driver Daemon v{}", env!("CARGO_PKG_VERSION"));

    let config = Arc::new(RwLock::new(TabletConfig::load_or_default()));
    let primary_device_info = Arc::new(RwLock::new(None::<DeviceInfo>));

    let dbus_service = HuionDbusService::new(config.clone(), primary_device_info.clone());
    let _conn = match zbus::connection::Builder::session() {
        Ok(builder) => match builder.name("org.wayland.HuionTablet") {
            Ok(builder) => match builder.serve_at("/org/wayland/HuionTablet", dbus_service) {
                Ok(builder) => match builder.build().await {
                    Ok(conn) => {
                        info!("Registered D-Bus service at org.wayland.HuionTablet");
                        Some(conn)
                    }
                    Err(e) => {
                        warn!("Could not connect to D-Bus session bus: {}. Running headless.", e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Could not attach D-Bus interface: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Could not request D-Bus name: {}", e);
                None
            }
        },
        Err(e) => {
            warn!("Could not build D-Bus connection: {}", e);
            None
        }
    };

    let virtual_tablet = match VirtualTablet::new() {
        Ok(vt) => {
            info!("Created virtual uinput tablet device: 'Huion HS64 Wayland Stylus/Pad'");
            Some(vt)
        }
        Err(e) => {
            error!("Failed to create /dev/uinput virtual device: {}. Check permissions.", e);
            None
        }
    };

    let vt_holder = Arc::new(TokioMutex::new(virtual_tablet));
    let (packet_tx, mut packet_rx) = mpsc::channel::<(PathBuf, Vec<u8>)>(256);
    let active_paths = Arc::new(StdMutex::new(HashSet::<PathBuf>::new()));

    let active_paths_monitor = active_paths.clone();
    let packet_tx_monitor = packet_tx.clone();
    let primary_info_holder = primary_device_info.clone();

    tokio::spawn(async move {
        loop {
            let devices = find_huion_devices();
            {
                let mut guard = primary_info_holder.write().await;
                *guard = devices.first().cloned();
            }

            for dev in devices {
                let mut paths = active_paths_monitor.lock().unwrap();
                if !paths.contains(&dev.path) {
                    paths.insert(dev.path.clone());
                    let path_to_read = dev.path.clone();
                    let tx = packet_tx_monitor.clone();
                    let paths_cleanup = active_paths_monitor.clone();

                    info!("Attaching input stream reader to {:?}", path_to_read);
                    tokio::task::spawn_blocking(move || {
                        let mut file = match File::open(&path_to_read) {
                            Ok(f) => f,
                            Err(e) => {
                                warn!("Could not open {:?}: {}", path_to_read, e);
                                if let Ok(mut set) = paths_cleanup.lock() {
                                    set.remove(&path_to_read);
                                }
                                return;
                            }
                        };

                        let mut buf = [0u8; 64];
                        loop {
                            match file.read(&mut buf) {
                                Ok(0) => break,
                                Ok(n) => {
                                    if tx.blocking_send((path_to_read.clone(), buf[..n].to_vec())).is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Read error on {:?}: {}", path_to_read, e);
                                    break;
                                }
                            }
                        }

                        if let Ok(mut set) = paths_cleanup.lock() {
                            set.remove(&path_to_read);
                        }
                    });
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    let config_processor = config.clone();
    let vt_processor = vt_holder.clone();

    tokio::spawn(async move {
        while let Some((source_path, data)) = packet_rx.recv().await {
            match parse_report(&data) {
                Ok(Some(report)) => {
                    let mut vt_guard = vt_processor.lock().await;
                    if let Some(ref mut vt) = *vt_guard {
                        let current_cfg = config_processor.read().await;
                        match report {
                            TabletReport::Stylus(s) => {
                                let _ = vt.emit_stylus(s, &current_cfg);
                            }
                            TabletReport::ExpressKeys(k) => {
                                let _ = vt.emit_express_keys(k);
                            }
                            TabletReport::Raw(_) => {}
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::trace!("Packet parse note from {:?}: {}", source_path, e);
                }
            }
        }
    });

    info!("Daemon running. Awaiting hardware events or termination (Ctrl+C)...");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down Huion Wayland Daemon.");

    Ok(())
}
```

### `crates/huion-gui/Cargo.toml`
```toml
[package]
name = "huion-gui"
version = "0.1.0"
edition = "2021"
description = "GTK4 and Libadwaita settings application for Huion Wayland"
license = "MIT OR Apache-2.0"

[dependencies]
huion-proto = { path = "../huion-proto" }
gtk4 = "0.9"
libadwaita = "0.7"
zbus = "5.5"
tokio = { version = "1.0", features = ["rt-multi-thread"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### `crates/huion-gui/src/main.rs`
```rust
use std::cell::RefCell;
use std::rc::Rc;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use adw::{ActionRow, Application, ApplicationWindow, ComboRow, HeaderBar, PreferencesGroup, PreferencesPage};
use gtk4::{Button, DrawingArea, Orientation as GtkOrientation, StringList};

use huion_proto::bezier::PressureCurve;
use huion_proto::config::{Orientation, TabletConfig};

const APP_ID: &str = "io.github.nirusaki.HuionWayland";

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let config = Rc::new(RefCell::new(TabletConfig::load_or_default()));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Huion HS64 Settings (Native Wayland)")
        .default_width(720)
        .default_height(680)
        .build();

    let header_bar = HeaderBar::new();
    let apply_btn = Button::builder()
        .label("Apply Settings")
        .css_classes(["suggested-action"])
        .build();
    header_bar.pack_end(&apply_btn);

    let page = PreferencesPage::new();

    let dev_group = PreferencesGroup::builder()
        .title("Device Status")
        .description("Hardware connection and identity under Wayland")
        .build();

    let dev_row = ActionRow::builder()
        .title("Huion HS64")
        .subtitle("Vendor ID: 0x256c | Direct Kernel Uinput Active")
        .build();
    dev_group.add(&dev_row);
    page.add(&dev_group);

    let map_group = PreferencesGroup::builder()
        .title("Display and Orientation")
        .description("Coordinate mapping and left-handed mode")
        .build();

    let orient_strings = StringList::new(&[
        "Normal (0°)",
        "Rotate 90° (Portrait)",
        "Rotate 180° (Left-Handed Mode)",
        "Rotate 270° (Inverted Portrait)",
    ]);
    let orient_row = ComboRow::builder()
        .title("Tablet Orientation")
        .model(&orient_strings)
        .selected(match config.borrow().orientation {
            Orientation::Normal => 0,
            Orientation::Rotate90 => 1,
            Orientation::Rotate180 => 2,
            Orientation::Rotate270 => 3,
        })
        .build();
    map_group.add(&orient_row);
    page.add(&map_group);

    let pressure_group = PreferencesGroup::builder()
        .title("Stylus Pressure Curve")
        .description("PW100 8192-level cubic Bezier calibration")
        .build();

    let pressure_presets = StringList::new(&["Linear (Default)", "Soft (Light Touch)", "Hard (Firm Stroke)"]);
    let preset_row = ComboRow::builder()
        .title("Pressure Curve Profile")
        .model(&pressure_presets)
        .selected(0)
        .build();
    pressure_group.add(&preset_row);

    let drawing_area = DrawingArea::builder()
        .content_width(600)
        .content_height(140)
        .css_classes(["card"])
        .build();

    let points: Rc<RefCell<Vec<(f64, f64)>>> = Rc::new(RefCell::new(Vec::new()));
    let points_clone = points.clone();

    drawing_area.set_draw_func(move |_, cr, _width, height| {
        cr.set_source_rgb(0.95, 0.95, 0.97);
        let _ = cr.paint();

        cr.set_source_rgb(0.2, 0.5, 0.9);
        cr.set_line_width(4.0);

        let pts = points_clone.borrow();
        if pts.len() >= 2 {
            cr.move_to(pts[0].0, pts[0].1);
            for pt in pts.iter().skip(1) {
                cr.line_to(pt.0, pt.1);
            }
            let _ = cr.stroke();
        }

        cr.set_source_rgb(0.5, 0.5, 0.5);
        cr.move_to(20.0, height as f64 - 20.0);
        let _ = cr.show_text("Pressure Test Area: draw here to test stroke response");
    });

    let drag = gtk4::GestureDrag::new();
    let pts_drag = points.clone();
    let da_clone = drawing_area.clone();
    drag.connect_drag_update(move |gesture, offset_x, offset_y| {
        if let Some((start_x, start_y)) = gesture.start_point() {
            pts_drag.borrow_mut().push((start_x + offset_x, start_y + offset_y));
            da_clone.queue_draw();
        }
    });
    drawing_area.add_controller(drag);

    let clear_btn = Button::builder().label("Clear Test Canvas").build();
    let pts_clear = points.clone();
    let da_clear = drawing_area.clone();
    clear_btn.connect_clicked(move |_| {
        pts_clear.borrow_mut().clear();
        da_clear.queue_draw();
    });

    let canvas_box = gtk4::Box::builder()
        .orientation(GtkOrientation::Vertical)
        .spacing(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    canvas_box.append(&drawing_area);
    canvas_box.append(&clear_btn);

    pressure_group.add(&canvas_box);
    page.add(&pressure_group);

    let keys_group = PreferencesGroup::builder()
        .title("Express Keys (4 Physical Buttons)")
        .description("Hardware button action bindings")
        .build();

    let key1_row = ActionRow::builder().title("Button 1 (Top)").subtitle("Undo (Ctrl + Z)").build();
    let key2_row = ActionRow::builder().title("Button 2").subtitle("Redo (Ctrl + Shift + Z)").build();
    let key3_row = ActionRow::builder().title("Button 3").subtitle("Brush Tool (B)").build();
    let key4_row = ActionRow::builder().title("Button 4 (Bottom)").subtitle("Eraser Tool (E)").build();

    keys_group.add(&key1_row);
    keys_group.add(&key2_row);
    keys_group.add(&key3_row);
    keys_group.add(&key4_row);
    page.add(&keys_group);

    let config_apply = config.clone();
    let orient_row_clone = orient_row.clone();
    let preset_row_clone = preset_row.clone();

    apply_btn.connect_clicked(move |_| {
        let mut cfg = config_apply.borrow_mut();
        cfg.orientation = match orient_row_clone.selected() {
            0 => Orientation::Normal,
            1 => Orientation::Rotate90,
            2 => Orientation::Rotate180,
            3 => Orientation::Rotate270,
            _ => Orientation::Normal,
        };

        cfg.pressure_curve = match preset_row_clone.selected() {
            1 => PressureCurve::new(0.5, 0.1, 0.8, 0.5),
            2 => PressureCurve::new(0.2, 0.5, 0.5, 0.9),
            _ => PressureCurve::default(),
        };

        let _ = cfg.save();

        let cfg_json = serde_json::to_string(&*cfg).unwrap_or_default();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Ok(connection) = zbus::Connection::session().await {
                    let _ = connection
                        .call_method(
                            Some("org.wayland.HuionTablet"),
                            "/org/wayland/HuionTablet",
                            Some("org.wayland.HuionTablet"),
                            "SetConfig",
                            &(cfg_json,),
                        )
                        .await;
                }
            });
        });
    });

    let main_box = gtk4::Box::builder().orientation(GtkOrientation::Vertical).build();
    main_box.append(&header_bar);
    main_box.append(&page);

    window.set_content(Some(&main_box));
    window.present();
}
```
