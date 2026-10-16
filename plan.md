# Architecture & Implementation Plan: Native Wayland Driver & Control Panel for Huion HS64

**Target Device**: Huion HS64 (`idVendor=0x256c`, Models: `HUION_T21d`, `HUION_T225`, `HUION_T260`)  
**Target Environment**: Native Wayland (GNOME/Mutter, KDE Plasma 6/KWin, Sway/wlroots, Hyprland)  
**Implementation Stack**: Rust (Daemon, Parser, Protocol, Uinput) + GTK4 / Libadwaita (Frontend UI)  
**Output Repository**: `Nirusaki-Malaal/HuionHS64-Wayland`  

---

## 1. Project Overview & Architectural Topology

The legacy Huion Linux driver (`v15.0.0.193`) fails under Wayland because it couples coordinate mapping to XRandR, cursor control to `XWarpPointer`, hotkey injection to `xdotool` / `libxdo` (`XTest`), and shortcut binding to `XRecord`.

This project implements a clean-room, native-Wayland driver suite in Rust that eliminates all X11 dependencies:

```
┌────────────────────────────────────────────────────────┐
│              Hardware Layer (Huion HS64)               │
│     Raw USB HID / /dev/hidraw* (Vendor ID: 0x256c)     │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
┌────────────────────────────────────────────────────────┐
│             huion-wayland-daemon (Rust)                │
│ ┌────────────────────────────────────────────────────┐ │
│ │ 1. Hidraw Reader & Hotplug Netlink Monitor         │ │
│ ├────────────────────────────────────────────────────┤ │
│ │ 2. Packet Parser (HS64 Report 0x07/0x08/0x09)      │ │
│ ├────────────────────────────────────────────────────┤ │
│ │ 3. Coordinate Transformation & Display Mapper      │ │
│ │    (wl_output / DRM Connector geometry / Rotation) │ │
│ ├────────────────────────────────────────────────────┤ │
│ │ 4. Cubic Bezier Pressure Calibration Curve Engine  │ │
│ ├────────────────────────────────────────────────────┤ │
│ │ 5. /dev/uinput Virtual Digitizer & Pad Emitter     │ │
│ ├────────────────────────────────────────────────────┤ │
│ │ 6. D-Bus Service (org.wayland.HuionTablet)         │ │
│ └────────────────────────────────────────────────────┘ │
└──────────────────────────┬─────────────────────────────┘
                           │ D-Bus IPC
                           ▼
┌────────────────────────────────────────────────────────┐
│              huion-wayland-gui (GTK4/Libadwaita)       │
│  - Interactive Pen Pressure Curve Visualizer & Test    │
│  - Multi-Monitor Active Area Mapping Canvas            │
│  - 4 Express Keys Hotkey & Action Mapping             │
│  - Orientation / Left-Handed Mode Toggle (0/90/180/270)│
│  - Profile Management (TOML in XDG_CONFIG_HOME)        │
└────────────────────────────────────────────────────────┘
```

---

## 2. Reverse-Engineered Protocol Specification (Huion HS64)

From decompilation of `libTabletSession.so` and `libhuionhid.so`:

### 2.1 USB Identification
- **Vendor ID**: `0x256c` (Huion / UC-Logic)
- **Product ID**: `0x006d` / `0x006b` / `0x0064`
- **Supported Revisions**: `HUION_T21d`, `HUION_T225`, `HUION_T260`

### 2.2 Stylus Event Packet (Report ID `0x09` / `0x07`)
Raw 10-to-12-byte packet structure:
```
Byte 0:  Report ID (0x07, 0x08, or 0x09)
Byte 1:  Status Flags:
         Bit 0: Tip Switch (Touching surface)
         Bit 1: Barrel Switch 1 (Lower stylus button)
         Bit 2: Barrel Switch 2 (Upper stylus button / Eraser)
         Bit 4: In-Range (Stylus detected by digitizer loop)
Bytes 2-3:  X Coordinate (Little Endian, uint16, range: 0..40000, 5080 LPI)
Bytes 4-5:  Y Coordinate (Little Endian, uint16, range: 0..25000, 5080 LPI)
Bytes 6-7:  Pressure (Little Endian, uint16, range: 0..8191, PW100 passive EMR)
Bytes 8-9:  Reserved
Byte 10:    Tilt X (Int8, -60° to +60°, if supported by revision)
Byte 11:    Tilt Y (Int8, -60° to +60°, if supported by revision)
```

### 2.3 Express Key Packet (Report ID `0x08` / `0x07`)
Raw 8-byte packet structure:
```
Byte 0:  Report ID (0x08)
Byte 1:  Key Bitmask:
         0x01: Express Key 1 (Top button)
         0x02: Express Key 2 (Second button)
         0x04: Express Key 3 (Third button)
         0x08: Express Key 4 (Bottom button)
Bytes 2-7: 0x00 (Padding)
```

---

## 3. Wayland Native Technical Design

### 3.1 Input Forwarding: `/dev/uinput`
We bypass X11 completely by generating kernel-level virtual input devices:
1. **Digitizer Tablet Device**:
   - `EV_ABS`: `ABS_X` (max 40000), `ABS_Y` (max 25000), `ABS_PRESSURE` (max 8191), `ABS_TILT_X`, `ABS_TILT_Y`.
   - `EV_KEY`: `BTN_TOOL_PEN`, `BTN_TOUCH`, `BTN_STYLUS`, `BTN_STYLUS2`.
   - Prop: `INPUT_PROP_DIRECT`.
2. **Pad Device (Express Keys)**:
   - `EV_KEY`: `BTN_0`, `BTN_1`, `BTN_2`, `BTN_3`.
   - Directly consumed by `libinput` and compositors (GNOME, KDE, Sway) as native tablet pad buttons.

### 3.2 Display Mapping & Geometry Discovery
Without XRandR, monitor resolution and positions are discovered via:
1. **DRM / KMS Interface**: Reading `/sys/class/drm/card*-*/modes` and status directly in the daemon for headless/display-agnostic fallback.
2. **Wayland Client Protocol**: Using `wayland-client` (`wl_output` and `xdg_output_manager_v1`) in the GTK4 application to retrieve per-output physical size, logical size, scale factor, and position.

### 3.3 Pressure Curve Engine
Cubic Bézier curve formulation with control points $P_0(0, 0)$, $P_1(x_1, y_1)$, $P_2(x_2, y_2)$, $P_3(1, 1)$:
$$B(t) = (1-t)^3 P_0 + 3(1-t)^2 t P_1 + 3(1-t) t^2 P_2 + t^3 P_3, \quad t \in [0, 1]$$
Output pressure: $P_{calibrated} = \text{clamp}(B(P_{raw} / 8191) \times 8191, 0, 8191)$.

---

## 4. Work Breakdown & Git Atomic Commit Strategy

Each milestone will be delivered as an atomic, standalone Git commit with descriptive conventional commit messages:

| Step | Commit Hash / Target | Description | Scope |
| :--- | :--- | :--- | :--- |
| **01** | `chore(repo): initialize workspace and cargo manifests` | Set up Rust Cargo workspace (`huion-proto`, `huion-daemon`, `huion-gui`), `.gitignore`, `LICENSE` (GPLv3), and directory scaffolding. | Root |
| **02** | `feat(proto): implement HS64 packet decoding and configuration schema` | Data structures for Stylus reports, Express Key masks, TOML configuration schema, serialization/deserialization. | `huion-proto` |
| **03** | `feat(daemon): implement hidraw driver reader and uevent hotplug monitor` | Asynchronous reader over `/dev/hidraw*` with Netlink hotplug detection for VID `0x256c`. | `huion-daemon` |
| **04** | `feat(daemon): implement Linux uinput virtual tablet and pad emission` | Virtual device creation via `nix`/`ioctl` for `/dev/uinput` with full pressure and stylus button capabilities. | `huion-daemon` |
| **05** | `feat(daemon): implement coordinate mapping, orientation, and Bezier curve engine` | Active area scaling, aspect ratio locking, 0/90/180/270 degree rotation, and cubic Bezier curve math. | `huion-daemon` |
| **06** | `feat(daemon): implement D-Bus IPC service interface` | Expose `org.wayland.HuionTablet` D-Bus service for runtime reconfiguration from GUI. | `huion-daemon` |
| **07** | `test(e2e): implement mock hidraw feeder and evdev verification test suite` | End-to-end integration tests simulating raw hardware bytes and verifying `evdev` events. | `tests/` |
| **08** | `feat(gui): scaffold GTK4 / Libadwaita Wayland application` | Modern desktop application using `gtk4` and `libadwaita` for tablet settings. | `huion-gui` |
| **09** | `feat(gui): implement pressure curve editor, display mapper, and key assignment` | Interactive Bezier curve canvas with live test scratchpad, monitor selector, and hotkey binder. | `huion-gui` |
| **10** | `docs: add comprehensive README.md and documentation.md` | User documentation, installation instructions, udev rules, systemd service, and troubleshooting. | Root |
| **11** | `ci/release: initialize GitHub repository and push via gh` | Create remote repository `Nirusaki-Malaal/HuionHS64-Wayland` via `gh` CLI and push commits. | GitHub |

---

## 5. End-to-End Testing & Verification Framework

1. **Virtual Hardware Mocking**:
   - Create a pseudo-terminal or Unix socket pair that mimics `/dev/hidraw` to stream synthetic HS64 packets.
2. **Event Verification**:
   - Use `evdev-rs` to open the emitted `/dev/uinput` node in the test process.
   - Inject known raw bytes:
     - Hover at $(X=10000, Y=10000, P=0)$.
     - Press with pressure $4096$ and barrel switch 1.
     - Express key 1 press and release.
   - Assert that emitted `evdev` events strictly match calibrated expectations.
3. **Wayland Output Simulation**:
   - Verify coordinate calculations against multi-monitor layouts (e.g. 1920x1080 primary + 2560x1440 secondary at scale 1.5).

---

## 6. Deployment & System Integration Artifacts

1. **Udev Rules (`99-huion-wayland.rules`)**:
   - Automatically tag Huion devices (`256c:*`), grant permissions to `uaccess` group, and trigger systemd user service.
2. **Systemd User Service (`huion-wayland.service`)**:
   - Background daemon management starting automatically with the Wayland session (`default.target`).
3. **Desktop Entry (`io.github.nirusaki.HuionWayland.desktop`)**:
   - Standard XDG desktop entry for application launcher integration.
