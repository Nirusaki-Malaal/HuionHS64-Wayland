# Current investigation — 2026-10-16

This section supersedes conflicting historical claims below. Fresh Ghidra
analysis, connected descriptors, actual reports and kernel/libinput verification
are recorded in [vendor-analysis.md](research/vendor-analysis.md),
[hardware-evidence.json](research/hardware-evidence.json), and
[validation.md](research/validation.md).

Confirmed corrections:

- Standard report 0x0a has X/Y ranges 32000/20400. Preserve saved coordinate
  settings by normalizing these into canonical 40000/25000.
- Its descriptor pressure max 4095 is misleading: live contact reports reached
  5473. Keep nominal 0–8191 raw pressure; scaling from 4095 clips firm strokes.
  Initial descriptor-based scaling was rejected after live evidence contradicted it.
- Standard proximity is bit6; clearing proximity/touch clears pressure.
- Vendor 0x08 is multiplexed, **not** a second-byte express-key mask. Ghidra
  confirms pen/key dispatch and USB string descriptor initialization.
- All four known keyboard usages were captured, along with their releases.
  Six HID usage slots must be parsed; unknown usages never imply Button4.
- The daemon previously ignored `express_keys` settings. It now emits validated
  configured actions with ordered, balanced shared modifiers and no duplicate
  joystick button actions.
- On this host, keyd's wildcard grabbed the physical HS64 keyboard and bypassed
  libinput ignore rules. Added `-256c:006f` to its ids, with backup at
  `/etc/keyd/default.conf.before-huion-wayland`.
- Permanent libinput suppression was replaced by runtime physical evdev grabs,
  a process lock, session cleanup and fail-closed injection. Stop/start and
  duplicate-daemon tests passed; kernel fallback is restored on stop.
- GUI now tests actual compositor pressure, has correct soft/firm response,
  native shortcut presets/custom actions, responsive layout and truthful status.
- Output/aspect mapping is not implemented; the daemon rejects these settings
  instead of claiming success. Tilt is not advertised. One standard-mode HS64
  is the supported hardware scope.

The report below is the earlier investigation retained for historical context;
its claimed hardware maxima, interface assignments and completed features must
not override fresh evidence above.

---

# Reverse Engineering Report: Huion Linux Driver (v15.0.0.193) & Wayland Compatibility

**Target Package**: `HuionTablet_LinuxDriver_v15.0.0.193.x86_64.deb`  
**Specific Device Focus**: Huion HS64 (`HUION_T21d` / `HUION_T225` / `HUION_T260`)  
**Architecture**: x86_64 ELF Linux  
**Date**: October 2026  

---

## 1. Architectural Answer: Does the Software Depend on X11, the Driver, or Both?

**The answer is BOTH, but rigidly partitioned across three distinct architectural tiers:**

| Component | Layer | Depends on X11? | Reason / Mechanism |
| :--- | :--- | :--- | :--- |
| **Kernel / Hardware Comms** | Kernel / USB HID | **NO** | Operates over USB (`idVendor=0x256c`) via raw HID reports, `libusb`/`libhnusb.so`, and standard Linux `/dev/hidraw*` or kernel `hid-uclogic`. Completely display-server agnostic. |
| **Driver Daemon (`huionCore`)** | Userland Service | **YES (Hard Dependency)** | Calls Xlib (`XOpenDisplay`, `XWarpPointer`, `XQueryPointer`), XRandR (`XRRGetMonitors`) for screen/monitor coordinate mapping, spawns `xdotool click 4/5`, and executes `xprop` on the X11 root window to monitor `_NET_ACTIVE_WINDOW` for application-specific hotkey profiles. |
| **Control Panel GUI (`huiontablet`)** | Qt5 Desktop App | **YES (Hard Dependency)** | Forces `QT_QPA_PLATFORM=xcb` in `huiontablet.sh`, dynamically links `libX11`, `libXrandr`, `libXtst`, `libQt5X11Extras`, and calls X11 `XRecord` APIs (`XRecordEnableContext`) to intercept keyboard shortcuts during express-key configuration. |
| **Helper Tools (`xdotool`, `libxdo`)** | CLI Helper | **YES (Hard Dependency)** | Shipped in deb and placed in `/bin/xdotool`; uses `XTestFakeKeyEvent` and `XTestFakeButtonEvent` to inject keystrokes and scroll wheel events. Fails entirely under native Wayland clients. |

---

## 2. Package Anatomy & File Breakdown

Unpacking the package (`dpkg-deb -R`) reveals:
```
unpacked/
├── DEBIAN/
│   ├── control         # Package metadata (Maintainer: chenbiao@huion.com)
│   └── postinst        # Copies bundled xdotool & libxdo.so.3 into /bin and /usr/lib
├── etc/
│   ├── udev/rules.d/20-huion.rules   # Grants 0666 permissions to uhid, uinput, USB 256c
│   └── xdg/autostart/huiontablet.desktop
└── usr/
    ├── lib/
    │   ├── huiontablet/
    │   │   ├── detectSessionProtocol  # Bash script querying loginctl / XDG_SESSION_TYPE
    │   │   ├── huionCore              # Unstripped ELF daemon (contains debug symbols)
    │   │   ├── huionCore.sh           # LD_LIBRARY_PATH wrapper
    │   │   ├── huiontablet            # Unstripped Qt5 GUI application
    │   │   ├── huiontablet.sh         # Hardcodes export QT_QPA_PLATFORM=xcb
    │   │   ├── xdotool/               # Bundled xdotool executable and libxdo.so.3
    │   │   ├── libs/                  # Bundled Qt 5.14/5.15, ICU, and proprietary SOs
    │   │   │   ├── libhnusb.so        # Low-level USB abstraction
    │   │   │   ├── libhuionhid.so     # Linux /dev/uhid and /dev/uinput bridge
    │   │   │   ├── libTabletSession.so# Hardware tablet state machine & packet parser
    │   │   │   ├── libQtSessionLib.so # Qt D-Bus IPC library
    │   │   │   └── libcfgio.so        # Proprietary config reader/writer
    │   │   └── res/                   # Tablet database (tablet.cfg, user.cfg, sessiontype.txt)
    └── share/applications/huiontablet.desktop
```

---

## 3. Deep Decompilation: The Driver Daemon (`huionCore`)

The binary `huionCore` is an unstripped 64-bit ELF with full DWARF symbol tables.

### 3.1 Device Detection & Low-Level Input Forwarding
- **Hardware Hotplug**: In `EventDetect::detect`, the daemon creates a Netlink socket:
  ```c
  socket(AF_NETLINK, SOCK_RAW, NETLINK_KOBJECT_UEVENT); // socket(16, 3, 15)
  bind(fd, &sockaddr_nl, sizeof(sockaddr_nl)); // nl_groups = 1
  ```
- **Virtual Input via `/dev/uhid`**:
  `libhuionhid.so` contains `create_tablet_dev(int fd, const char* name, int product)`:
  It writes a `struct uhid_create2_req` directly to `/dev/uhid` using an embedded 122-byte HID report descriptor (`g_rdesc`):
  ```
  Usage Page (Digitizers 0x0D), Usage (Pen 0x02)
  Collection (Application)
    Report ID (0x09)
    Usage (Tip Switch), Usage (Barrel Switch), Usage (Invert), Usage (In Range)
    Usage (X) [0..40000], Usage (Y) [0..25000]
    Usage (Tip Pressure) [0..8191]
    Usage (Tilt X), Usage (Tilt Y)
  ```
- **Virtual Input via `/dev/uinput`**:
  Initializes a virtual mouse and keyboard named `"Virtual HUION Tablet Wheel Device"` via `/dev/input/uinput` for relative pointer motion and keyboard events.

### 3.2 Where X11 Hard Dependencies Break `huionCore`

#### 1. Display Enumeration & Coordinate Mapping (`hncb_enum_display_Xrandr`)
```c
// huionCore address 0x8482:
Display *dpy = XOpenDisplay(NULL);
XRRMonitorInfo *monitors = XRRGetMonitors(dpy, XRootWindow(dpy, screen), 1, &nmonitors);
// Extracts: monitors[i].x, monitors[i].y, monitors[i].width, monitors[i].height
```
- **The Wayland Flaw**: Under Wayland, XRandR queries talk to Xwayland. Xwayland either returns a single virtual bounding canvas or reports unscaled geometries that do not match Wayland output scaling factors or fractional scale boundaries. Without `$DISPLAY`, `XOpenDisplay` fails immediately.

#### 2. Absolute Cursor Tracking & Warping (`GetCursorPos` & `show_cursor_in_x11`)
```c
// huionCore address 0x15a6d:
XQueryPointer(dpy, root_win, &root_return, &child_return, &root_x, &root_y, ...);

// huionCore address 0x130da:
XWarpPointer(dpy, None, root_win, 0, 0, 0, 0, target_x, target_y);
XFlush(dpy);
XCloseDisplay(dpy);
```
- **The Wayland Flaw**: Global cursor polling (`XQueryPointer`) and arbitrary cursor warping (`XWarpPointer`) are **fundamentally illegal** under Wayland security doctrine. Wayland compositors do not allow unprivileged client applications to warp the pointer across windows or spy on the global cursor position outside their own surfaces.

#### 3. Active Window Detection for Application-Specific Profiles
```c
// String located at huionCore offset 0x33c50:
"xprop -id $(xprop -root | awk '/_NET_ACTIVE_WINDOW(WINDOW)/{print $NF}') | awk '/WM_CLASS/{print $NF}'"
```
- **The Wayland Flaw**: `huionCore` executes this shell pipeline via `popen()` every time the pen approaches the tablet to determine which application has focus (e.g. Krita, Blender, Inkscape, Photoshop via Wine). On Wayland, the root window does not track native Wayland client window handles in `_NET_ACTIVE_WINDOW`. `xprop` returns nothing, and per-app express key switching fails completely.

#### 4. Express Key & Wheel Injection
- For scroll wheel / zoom ring actions, `huionCore` executes:
  ```c
  popen("xdotool click 4", "r"); // Scroll Up
  popen("xdotool click 5", "r"); // Scroll Down
  ```
- **The Wayland Flaw**: `xdotool` sends synthetic X11 `XTestFakeButtonEvent` / `XTestFakeKeyEvent`. Native Wayland applications (running on Wayland protocols, not Xwayland) discard these events completely.

---

## 4. Reverse Engineering the Incomplete Wayland Prototype

Huion attempted to start adding Wayland support in driver version 15.0.0.193, but left it incomplete:

### 4.1 Protocol Detection Script (`detectSessionProtocol`)
Located at `/usr/lib/huiontablet/detectSessionProtocol`:
```bash
#!/bin/bash
# 1. Checks loginctl show-session <id> -p Type
# 2. Checks $XDG_SESSION_TYPE
# 3. Checks pgrep wayland
```

### 4.2 Daemon Detection Logic (`huion_is_wayland`)
In `huionCore` (offset `0x998d`):
1. `huion_is_wayland()` executes `/usr/lib/huiontablet/detectSessionProtocol` via `popen()`.
2. Compares output against string `"Wayland"`.
3. Writes the string `"Wayland"` or `"X11"` to `/usr/lib/huiontablet/res/sessiontype.txt`.
4. Returns `1` (true) if Wayland is detected, `0` (false) otherwise.

### 4.3 What Actually Changes When `huion_is_wayland() == 1`?
Decompilation of call sites reveals only two partial adaptations:
1. **Mouse Wheel Events (`send_wheel_event_in_wayland`)**:
   Instead of running `xdotool click 4`, it bypasses X11 and writes directly to the `/dev/uinput` device (`g_g + 0x138`):
   ```c
   struct input_event ev;
   ev.type = EV_REL; // 0x02
   ev.code = REL_WHEEL; // 0x08
   ev.value = val;
   write(uinput_fd, &ev, sizeof(ev));
   ev.type = EV_SYN;
   ev.code = SYN_REPORT;
   ev.value = 0;
   write(uinput_fd, &ev, sizeof(ev));
   ```
2. **Disables Mouse Function (`removeMouseFunctionOnWayland`)**:
   Calls `HnTabletThread::removeMouseFunctionOnWayland()`, which strips absolute mouse emulation because `XWarpPointer` crashes.
3. **What Huion Forgot to Fix**:
   - `hncb_enum_display_Xrandr` still runs `XOpenDisplay()` unconditionally!
   - `xprop` for window class detection is still executed!
   - `huiontablet.sh` still forces `QT_QPA_PLATFORM=xcb`!
   - `XRecord` in the GUI still crashes on Wayland!

---

## 5. Decompilation of the Control Panel GUI (`huiontablet`)

The UI is built with Qt 5.14.2 / QML and communicates with `huionCore` via D-Bus:
- **D-Bus Service**: `org.Huion.QtDBus.Msg`
- **D-Bus Path / Interface**: `org.Huion.QtDBus.Msg.core`
- **C++ Class**: `CoreReceiver::CoreMsgProc(HnDBusMsg const&)`

### Why the GUI Fails on Wayland:
1. **Hardcoded X11 Launcher**: `huiontablet.sh` forces `export QT_QPA_PLATFORM=xcb`. If Xwayland is disabled or missing, the process terminates immediately.
2. **Keybinding Snoop via XRecord**:
   When binding express keys in the GUI, `huiontablet` uses:
   ```c
   XRecordAllocRange();
   XRecordCreateContext(dpy, 0, &record_client, 1, &record_range, 1);
   XRecordEnableContext(dpy, context, RecordCallback, NULL);
   ```
   Wayland security prevents non-compositor applications from globally capturing keypresses. Under Wayland, the key-recording dialog hangs or captures nothing.
3. **XRandR Screen Geometry**:
   The interactive mapping screen calls `XRRGetScreenResourcesCurrent` and `XRRGetCrtcInfo` to render monitor bounding boxes. Under Wayland, this view cannot discern true Wayland scale-factor boundaries.

---

## 6. Device Profile: Huion HS64 Hardware Signature

Extracted from `/usr/lib/huiontablet/res/tablet.cfg` and `user.cfg`:
- **USB Vendor ID**: `0x256c` (Huion / UC-Logic)
- **Product Names / Internal Codes**:
  - `"HUION_T21d"`: Huion HS64 (Standard)
  - `"HUION_T225"`: Huion HS64 (Revision 2)
  - `"HUION_T260"`: Huion HS64 (Chips Special Revision)
- **Physical Specifications**:
  - Express Keys: 4 tactile buttons (`HKey1.png` to `HKey2.png` mappings)
  - Active Area: 160 mm x 102 mm (6.3 x 4.0 inches)
  - Resolution: 5080 LPI
  - Stylus Model: PW100 (Battery-free EMR)
  - Pressure Levels: 8192 levels (`0x0000` to `0x1FFF`)
  - Report Rate: 266 PPS

---

## 7. Engineering Blueprint for Full Wayland Compatibility

To make the Huion HS64 fully functional on modern Wayland environments (GNOME Mutter, KDE KWin, Sway/wlroots, Hyprland), there are two distinct engineering paths:

### Route A: Patching & Shimming the Proprietary Huion Stack
If retaining Huion's proprietary Qt GUI and daemon is required:
1. **Fix GUI Startup (`huiontablet.sh`)**:
   - Change `export QT_QPA_PLATFORM=xcb` to dynamically select:
     ```bash
     if [ "$XDG_SESSION_TYPE" = "wayland" ]; then
         export QT_QPA_PLATFORM=wayland;xcb
     else
         export QT_QPA_PLATFORM=xcb
     fi
     ```
2. **LD_PRELOAD Shim for `huionCore` (`libhuion_wayland_shim.so`)**:
   Intercept the offending X11 functions inside `huionCore`:
   - `XOpenDisplay`: When `$DISPLAY` is absent or in pure Wayland, return a dummy display struct rather than `NULL`.
   - `XRRGetMonitors`: Intercept and populate `XRRMonitorInfo` by querying Wayland outputs via `wl_output` or D-Bus (`org.gnome.Mutter.DisplayConfig` / KWin DBus).
   - `XWarpPointer`: Intercept and route to `/dev/uinput` absolute pointer coordinates.
   - `XQueryPointer`: Intercept and read last known coordinate from the tablet event stream.
3. **Replace `xdotool` with Native `uinput` / `wtype`**:
   Replace `/bin/xdotool` calls with native Wayland virtual input (`wtype`, `ydotool`, or emit via `libhuionhid.so`'s `/dev/uinput` device).
4. **Active Window Focus on Wayland**:
   Replace `xprop` query with Wayland-compatible compositor queries (e.g. `hyprctl activewindow`, `swaymsg -t get_tree`, or GNOME Shell DBus eval).

### Route B: Native Linux Tablet Stack (OpenTabletDriver / Linux `hid-uclogic`)
Because Huion HS64 hardware adheres strictly to UC-Logic USB protocol standards, the entire proprietary Huion userland driver is essentially redundant on modern Linux:
1. **Kernel Driver**:
   The mainline Linux kernel already contains the `hid-uclogic` module, which natively parses Huion HS64 packets and creates standard `evdev` nodes for both stylus and express keys.
2. **Wayland Native Handling**:
   - **GNOME (libinput + Mutter)**: Native `Settings -> Graphics Tablet` panel configures mapping, display assignment, pressure curves, and express keys directly over Wayland protocols.
   - **KDE Plasma (KWin)**: Plasma 6 includes dedicated Wayland tablet configuration supporting Huion devices via `libinput`.
   - **OpenTabletDriver (OTD)**: Open-source, daemon-based userland driver written in C#/.NET with full native Wayland coordinate mapping and custom GUI.
