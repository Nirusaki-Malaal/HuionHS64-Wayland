# Verification - 2026-10-16

These results distinguish static analysis, synthetic checks and live hardware.
They are not a claim that every possible driver bug or compositor is covered.

## Executed checks

| Check | Result |
| --- | --- |
| Ghidra focused vendor decompilation | Completed; hashes, ELF addresses and limitations in vendor-analysis.md |
| Protocol parser/curve/config tests | 19 passed, including captured pressure4377 and malformed keyboard reports |
| Daemon shortcut/frame unit tests | 4 passed |
| GUI behavior/geometry tests | 8 passed; pressure presets, shortcut presets/custom actions, sheet rotation/resize |
| Kernel uinput -> evdev integration | Passed explicitly with --include-ignored; pressure, proximity release, configured Undo and releases, single SYN |
| D-Bus live contract | Passed: config roundtrip, rejection of invalid area/key/output, unchanged settings, status/display schemas |
| Live service lifecycle | Passed: prompt SIGTERM, no leftover virtual devices, physical grab release and reacquisition, exactly one replacement pair |
| Second daemon | Rejected by session lock before another virtual device was created |
| Strict workspace Clippy | Passed using installed system cargo-clippy |
| Rust formatting | Passed using installed system cargo-fmt |
| Release workspace build | Passed; binaries installed under /usr/local/bin |
| Diagnostic capture utility | Passed short live discovery/capture smoke check |
| GTK rendering | Wide/narrow layout screenshots inspected under Xvfb; native Wayland launch also exercised; final separate pages/diagram/portrait views checked |

The kernel integration test initially detected keyd owning the shortcut device.
After the backed-up HS64 exclusion was installed, the same test passed. Its first
run also exposed asynchronous evdev permission assignment; the test now waits
boundedly for udev before opening its temporary devices. Test devices are grabbed
before emission so their synthetic shortcuts do not reach the desktop.

## Observed hardware

Connected device: USB vendor256c/product006f, three physical raw interfaces.
Descriptors and bounded actual report samples are retained in
[hardware-evidence.json](hardware-evidence.json).

- Standard pen0x0a reports reached raw pressure5473, disproving descriptor max4095.
- Known keyboard0x03 usages1d/2c/30/2f and zero-release packets were all observed.
- Libinput classified the replacement pen as a tablet (not a pointer device),
  and the shortcut device as keyboard/pointer with no relative motion emitted.
- A read-only libinput monitor observed thousands of tablet events with changing
  pressure, plus balanced Ctrl+Z, Ctrl+Shift+Z, B and E key events.
- The user confirmed pressure, buttons and pointer behavior were working.
- The user also confirmed exact physical order with cable up: Button1 upper-left,
  Button2 lower-left, Button3 upper-right, Button4 lower-right.

One early installed iteration scaled pressure using descriptor4095 and reached
libinput1.0 prematurely. After live reports contradicted that assumption, it was
corrected to retain nominal0-8191 raw pressure, covered by captured regression
and repeated kernel/build checks. The retained early libinput pressure summary
must not be mistaken for a final full-range calibration measurement.

Final review also fixed unrelated held mouse/pan actions being retriggered by
another button's modifiers. A regression holds each action through Ctrl+Z and
its release. Physical evdev inventory is now rechecked alongside hidraw paths,
and startup requires both pen and keyboard functions, addressing asynchronous
interface appearance rather than accepting any single physical node.

## Local system changes

- Installed release daemon/GUI, desktop entry, rules and user service.
- Earlier installed daemon/GUI/rules backed up as `.before-20261016-fixes` siblings.
- Added `-256c:006f` to `/etc/keyd/default.conf` and reloaded keyd; original saved
  at `/etc/keyd/default.conf.before-huion-wayland`.
- Rules clear the previous physical LIBINPUT_IGNORE_DEVICE setting. Live ownership
  now uses process-scoped evdev grabs.
- User service is running; its prior disabled autostart state was preserved.
- Temporary read-only capture/monitor processes were stopped after evidence collection.

## Scope still unverified or unsupported

Other firmware revisions/products, vendor0x08 mode negotiation, tilt, several
simultaneous tablets, output/aspect integration, every compositor, and every
painting app/brush configuration are outside the completed verification. The
GUI-only pen-sheet rotation is independent of actual tablet/system rotation.
A native Wayland pressure test depends on compositor tablet protocol delivery;
Xvfb visual checks cannot exercise physical stylus pressure.

## Final UI delivery and repeated checks

The follow-up UI request is implemented: named shortcut presets, dedicated large
pen workspace, local sheet rotation, and a cable-up visual/text button map.
Wide/narrow diagram and landscape/portrait screenshots were inspected. Final
workspace tests passed31 normal cases, plus the explicitly enabled kernel
integration case (32 total). Strict Clippy, formatting and release build passed.
Final installed binaries passed both live service scripts again. The lifecycle
test was corrected to wait for published connected readiness: virtual nodes can
become visible before physical grabs finish, so node count alone was a premature
readiness signal. Both physical nodes were owned after that readiness signal.

The final GUI was reopened on the user's native Wayland session. Example
screenshots are included under docs/images/. Service autostart remains disabled.
