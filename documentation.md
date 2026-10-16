# Technical Documentation: Huion HS64 Native Wayland Driver

**Package**: `huion-wayland`  
**Architecture**: Native Wayland Linux Driver & Settings Control Panel  
**Target Hardware**: Huion HS64 (`HUION_T21d`, `HUION_T225`, `HUION_T260`)  
**Language**: Rust (Memory-safe, zero-allocation packet processing)  
**GUI Framework**: GTK4 / Libadwaita (Native Wayland client)  

---

## 1. System Architecture & Component Interactions

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
                       ┌───────────────────────┴───────────────────────┐
                       │                                               │
                       ▼                                               │
┌──────────────────────────────────────────────┐                       │
│             huion-daemon                     │                       │
│  ┌─────────────────────────────────────────┐ │                       │
│  │ 1. hidraw reader & packet parser        │ │                       │
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

---

## 2. Reverse-Engineered USB HID Protocol Specification

Huion HS64 uses the UC-Logic hardware architecture communicating through USB interrupt transfer endpoints.

### 2.1 Stylus Packet (Report ID `0x07` / `0x09`)
- **Packet Length**: 10 to 12 bytes
- **Endianness**: Little-Endian (`from_le_bytes`)

| Byte Offset | Field Name | Data Type | Value Range / Description |
| :--- | :--- | :--- | :--- |
| `0` | Report ID | `u8` | `0x07`, `0x09`, or `0x0A` |
| `1` | Status Flags | `u8` | Bit 0: Touch (`0x01`)<br>Bit 1: Barrel Button 1 (`0x02`)<br>Bit 2: Barrel Button 2 / Eraser (`0x04`)<br>Bit 4: In-Range Proximity (`0x10`) |
| `2..3` | Absolute X | `u16` (LE) | `0 .. 40000` (5080 LPI, 160 mm active width) |
| `4..5` | Absolute Y | `u16` (LE) | `0 .. 25000` (5080 LPI, 102 mm active height) |
| `6..7` | Tip Pressure | `u16` (LE) | `0 .. 8191` (PW100 battery-free EMR sensor) |
| `8..9` | Reserved | `u16` | Vendor diagnostic bytes |
| `10` | Tilt X | `i8` | Signed angle: `-60° .. +60°` |
| `11` | Tilt Y | `i8` | Signed angle: `-60° .. +60°` |

### 2.2 Express Key Packet (Report ID `0x08`)
- **Packet Length**: 8 bytes

| Byte Offset | Field Name | Description |
| :--- | :--- | :--- |
| `0` | Report ID | `0x08` |
| `1` | Key Bitmask | Bit 0 (`0x01`): Button 1 (Top)<br>Bit 1 (`0x02`): Button 2<br>Bit 2 (`0x04`): Button 3<br>Bit 3 (`0x08`): Button 4 (Bottom) |
| `2..7` | Padding | `0x00` |

---

## 3. Mathematical Foundations

### 3.1 Cubic Bézier Pressure Curve
Pressure values are transformed using a parametric cubic Bézier curve defined by endpoints $P_0(0, 0)$, $P_3(1, 1)$ and adjustable control points $P_1(x_1, y_1)$, $P_2(x_2, y_2)$:

$$B_x(t) = 3(1-t)^2 t x_1 + 3(1-t) t^2 x_2 + t^3$$
$$B_y(t) = 3(1-t)^2 t y_1 + 3(1-t) t^2 y_2 + t^3$$

Given normalized raw pressure $p_{in} \in [0.0, 1.0]$:
1. Binary search finds $t \in [0.0, 1.0]$ such that $|B_x(t) - p_{in}| < 10^{-4}$.
2. Output pressure factor is $p_{out} = B_y(t)$.
3. Calibrated pressure emitted to `/dev/uinput`: $P = \text{round}(p_{out} \times 8191)$.

### 3.2 Coordinate Transformation & Orientation
The coordinate engine maps physical coordinates $(X, Y)$ within the defined active area $[x_0, x_0 + w] \times [y_0, y_0 + h]$ onto the full normalized digitizer bounds:

- **Normal (0°)**:
  $$(X', Y') = (\text{norm}(X), \text{norm}(Y))$$
- **Rotate 180° (Left-Handed Mode)**:
  $$(X', Y') = (X_{max} - \text{norm}(X), Y_{max} - \text{norm}(Y))$$
- **Rotate 90° (Portrait Clockwise)**:
  $$(X', Y') = (X_{max} - \text{norm}(Y), \text{norm}(X))$$
- **Rotate 270° (Portrait Counter-Clockwise)**:
  $$(X', Y') = (\text{norm}(Y), Y_{max} - \text{norm}(X))$$

---

## 4. D-Bus IPC Specification (`org.wayland.HuionTablet`)

The daemon exports the `org.wayland.HuionTablet` interface on the D-Bus Session Bus:

### Methods

#### `GetConfig() -> String`
Returns the active tablet configuration serialized as a JSON string.

#### `SetConfig(json_str: String) -> Boolean`
Validates, updates runtime parameters, and persists the configuration to `$XDG_CONFIG_HOME/huion-wayland/config.toml`. Returns `true` on success.

#### `GetDeviceInfo() -> String`
Returns device connectivity state and hardware metadata:
```json
{
  "connected": true,
  "name": "Huion HS64",
  "vendor_id": 9580,
  "product_id": 109,
  "path": "/dev/hidraw2"
}
```

#### `GetDisplays() -> String`
Returns active DRM display outputs and modes:
```json
[
  {
    "name": "eDP-1",
    "width": 1920,
    "height": 1080,
    "connected": true
  }
]
```

---

## 5. Security & Permission Architecture

1. **Udev Tagging**:  
   `data/70-huion-wayland.rules` applies `TAG+="uaccess"` and `GROUP="input"` to Huion devices and `/dev/uinput`. This grants read/write permissions directly to the active systemd login session user without running the daemon as root.
2. **Wayland Boundary Compliance**:  
   No X11 client APIs, no window property sniffing, no global pointer warping, and no keylogging hooks. Input is emitted cleanly through the kernel input subsystem.
