# Vendor binary evidence: pressure, report routing and initialization

Analysis performed 2026-10-16 against the supplied v15.0.0.193 package.
These are fresh static observations, separate from historical claims in
`research.md`. The vendor executables were not run and no USB mode change was
sent to the tablet during this analysis.

## Identity and method

| Artifact | SHA-256 |
| --- | --- |
| `HuionTablet_LinuxDriver_v15.0.0.193.x86_64.deb` | `cc563c2358dd0ba13c245f97341d20fecfa8509bfa5c75c8258c895f48b6fa02` |
| `unpacked/usr/lib/huiontablet/huionCore` | `fbf5a8260df494ca66981de2e5937102b5b7bfa1aaf93c84e4943600a6ec94ea` |
| `libs/libTabletSession.so` | `834509de9e0c099cf4211c12fffbba4f22b7c143a12e5596f058bc453d99b052` |
| `libs/libhnusb.so` | `826914e46adb3787da615ffe3e0afe82545af48f5af1ec0b27f2531ff70e31e8` |
| `libs/libhuionhid.so` | `fd6f344c6cd1056b08cc18b801c8b7ed4a58586308de3c8ea313553a800e0345` |

All library paths above are relative to `unpacked/usr/lib/huiontablet/`.
`nm -C` located retained function names; GNU `objdump -d -C` established field
loads and USB arguments; existing `/opt/ghidra/support/analyzeHeadless` produced
focused pseudocode for `huionCore` and `libhnusb.so`. No REA MCP tools were
registered in the active session, so the existing Ghidra installation was used
without changing MCP configuration or installing software.

Addresses below are ELF virtual addresses as shown by `nm`/`objdump`. Ghidra
loaded these PIE/shared objects with a `0x100000` base: e.g. its `0011b2a7`
corresponds to ELF address `0x1b2a7`. Disassembly corroborated the important
loads; decompiler local types/names are not assumed to be original source.
Ghidra reported GCC exception-region disassembly errors during automatic
analysis; the focused functions below produced pseudocode and the conclusions
are bounded to those functions, not a complete reconstruction of the driver.

## Finding ledger

| ID | Observation and evidence | Confidence / limitation |
| --- | --- | --- |
| V01 | `huionCore:is_invalid_report` at `0x1b1ef` rejects report IDs other than `0x08`; `loopSync` at `0xf152` calls this before dispatch. | Direct static evidence for vendor synchronous path, not proof of live tablet mode. |
| V02 | `report_type` at `0x1b209` reads byte 1. `HnTabletThread::dispatch` at `0xed82` routes type `<0x91` or `0xa0`/`0xa1` to pen; `0xe0`/`0xe2` to hardware keys; `0xe1` to touch; `0xe3` to macro; `0xf0`/`0xf1` to ring/slider/dial logic. | Direct decompilation, corroborated parser disassembly. Report ID alone cannot select key interpretation. |
| V03 | `report_x` at `0x1b21f` constructs `b2 | b3<<8 | b8<<16`; `report_y` at `0x1b263` constructs `b4 | b5<<8 | b9<<16`. | Direct loads; generic vendor format supports coordinates beyond 16 bits. HS64-specific high-byte use not established. |
| V04 | `report_p` at `0x1b2a7` loads unsigned little-endian 16-bit pressure from bytes 6..7. `parse_maxp` at `0x1b445` reads the same offsets in the stripped capability descriptor. | Direct loads; these offsets do not establish the standard `0x0a` pressure maximum. |
| V05 | `HnDoerPen::handle` at `0x13c4e` scales pressure by a capability-derived maximum before `hn_calibrate_press` at `0x1bdae`; the calibration paths use maximum `0x1fff`. It derives a tip-state field from calibrated pressure being nonzero. | Direct decompilation. Application-specific branches can bypass calibration. |
| V06 | `report_hbtn_id` at `0x1b2bd` constructs a little-endian 32-bit value from bytes 4..7. `HnDoerHkey::handle` at `0x17db6` compares this with saved key state and calls `handleHek` at `0x1805a`. | Direct evidence for vendor hardware-key type packets; no evidence that arbitrary `0x08` bytes are an HS64 4-bit key mask. |
| V07 | `initTabletInfo` at `0xf994` issues synchronous requests in order `201`, `200`, `202`, each with 256-byte buffers; model parsing is `parse_cmd_resp_201` at `0x1b618`, capability parsing `parse_cmd_resp_200` at `0x1b537`. | Direct decompilation; asynchronous devices have a separate request chain. |
| V08 | `libTabletSession.so:HnUsb::getStringDesc` at `0xc56a` writes the requested index into byte 0, calls `hid_get_feature_report`, and removes the first two returned bytes. Despite its name, `libhnusb.so:hid_get_feature_report` at `0x64ee` invokes `libusb_control_transfer(handle, 0x80, 0x06, 0x0300 | index, 0, buffer, 256, 0)`. | Direct Ghidra pseudocode and disassembly. This is USB GET_DESCRIPTOR with string descriptor type 3, **not** HID GET_REPORT / `HIDIOCGFEATURE`. |
| V09 | `libTabletSession.so:HnUsb::startRead` at `0xc540` simply returns zero. | Direct disassembly; do not invent an initialization write based on this function name. |
| V10 | `HnDoerPen::send_pen_data` at `0x1520a` fills output coordinates/pressure and calls `send_uinput_pen_event`; `libhuionhid.so:send_uinput_pen_event` at `0x2c44` emits `ABS_PRESSURE`, `ABS_X`, `ABS_Y`, `ABS_TILT_X/Y` and a synchronization event. Tool proximity uses a separate function `send_uinput_btn_tool_pen_event` at `0x2d21`. | Static corroboration that stylus pressure travels as an absolute tablet axis. Does not establish correct libinput classification of the replacement device. |

## Consequences for the replacement

A standard digitizer report (`0x0a`) and vendor report (`0x08`) are distinct
protocols. Using raw bit meanings from one for the other, or interpreting all
vendor reports as express keys, is unsupported. In particular the standard HID
report descriptor is the authority for standard-mode coordinate and pressure
ranges and proximity flags; the vendor driver instead obtains capabilities
through descriptor 200.

A proposed vendor initialization path must use USB descriptor requests, parse
and validate capabilities, and select the matching report parser together. A
function named `hid_get_feature_report` in this package is misleading: copying
its apparent name into an ioctl-based implementation would use the wrong
transfer. Static analysis proves the requests, not which exact request changes
this HS64 revision's firmware mode or whether the kernel has already made
those requests. Captured report data before/after requests is still needed to
make that claim.

Keep standard mode working when vendor initialization has not been confirmed.
Do not attach two competing pointer producers or infer that pressure has been
hardware-tested merely because parser/unit tests pass. Intermittent duplicate
pointers require live evdev/libinput/compositor observation, outside this
static investigation.

## Reproduction

The helper in `tools/FocusedDecompile.java` writes selected pseudocode to an
operator-chosen local output file. Do not commit proprietary decompilation dumps.
For example, from this repository, with an existing Ghidra installation:

```sh
mkdir -p /tmp/huion-review
/opt/ghidra/support/analyzeHeadless /tmp/huion-review Huion \
  -import unpacked/usr/lib/huiontablet/huionCore \
  -scriptPath "$PWD/research/tools" \
  -postScript FocusedDecompile.java /tmp/huion-core-review.c \
  '(dispatch|initTabletInfo|report_p|report_hbtn_id|report_type|report_x|report_y|parse_maxp|handle|send_pen_data|hn_calibrate_press)'
```

For the USB transfer, import `unpacked/usr/lib/huiontablet/libs/libhnusb.so`
into a separate project and select `hid_get_feature_report`. Corroborate by
running `objdump -d -C --start-address=0x64ee --stop-address=0x6593` on that
library. This review intentionally does not change device state.

## Live evidence supplement

The connected standard-mode HID descriptor advertises pressure max4095, but
subsequent captured0x0a contact reports reached5473. The firmware contradicts
its pressure metadata. Keep observed raw pressure in the nominal0-8191 range;
do not normalize pressure using4095. This does not change the vendor-mode
capability-query findings above. See hardware-evidence.json for actual samples.
