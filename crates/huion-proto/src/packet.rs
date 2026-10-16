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

// Canonical output range, retained for compatibility with saved active areas.
// The connected HS64 standard digitizer (report 0x0a) advertises different
// native ranges; normalize those reports before the rest of the pipeline.
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
        // Standard digitizer 0x0a and legacy pen layouts 0x07/0x09.
        0x07 | 0x09 | 0x0a => {
            if data.len() < 8 {
                return Err(ProtocolError::PacketTooShort {
                    expected: 8,
                    actual: data.len(),
                });
            }

            let status = data[1];
            let mut touch = (status & 0x01) != 0;
            let barrel_switch = (status & 0x02) != 0;
            let eraser = (status & 0x04) != 0;
            // The live 0x0a HID descriptor puts In Range in bit 6; bit 4
            // belongs to the preceding flags, not proximity.
            let in_range = if report_id == 0x0a {
                (status & 0x40) != 0
            } else {
                (status & 0x40) != 0 || (status & 0x10) != 0 || touch || barrel_switch || eraser
            };

            let mut x = u16::from_le_bytes([data[2], data[3]]);
            let mut y = u16::from_le_bytes([data[4], data[5]]);
            let mut pressure = u16::from_le_bytes([data[6], data[7]]);
            if report_id == 0x0a {
                // Captured /sys/class/hidraw/hidraw8/device/report_descriptor:
                // X max 0x7d00, Y max 0x4fb0, pressure max 0x0fff.
                x = normalize(x, 32000, HS64_MAX_X);
                y = normalize(y, 20400, HS64_MAX_Y);
                pressure = normalize(pressure, 4095, HS64_MAX_PRESSURE);
                touch &= in_range;
            }
            // Avoid stale pressure generating marks on hover or proximity out.
            if !touch || !in_range {
                pressure = 0;
            }

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

        // Interface 0 advertises 88 opaque vendor bits, not a button mask.
        // Preserve these reports until the vendor protocol is identified.
        0x08 => Ok(Some(TabletReport::Raw(data.to_vec()))),

        // Express Key Packet (Report ID 0x03 USB HID Keyboard reports from HS64 interface 2)
        0x03 => {
            if data.len() < 8 {
                return Err(ProtocolError::PacketTooShort {
                    expected: 8,
                    actual: data.len(),
                });
            }

            // Unlike a boot keyboard, this descriptor has no reserved byte:
            // report ID, modifiers, then six key usages. All usages are held
            // keys, so process the entire array (including simultaneous keys).
            let usages = &data[2..8];
            if usages.iter().any(|usage| (1..=3).contains(usage)) {
                // HID ErrorRollOver / POSTFail / ErrorUndefined conveys no
                // usable state. Keep the previous button state until recovery.
                return Ok(None);
            }
            let mask = usages.iter().fold(0, |mask, usage| {
                mask | match usage {
                    0x1d => 1, // Z / Undo
                    0x2c => 2, // Space / Hand
                    0x30 => 4, // ] / Brush+
                    0x2f => 8, // [ / Brush-
                    _ => 0,    // Modifiers and unknown usages are not button 4.
                }
            });
            Ok(Some(TabletReport::ExpressKeys(ExpressKeyPacket {
                raw_mask: mask,
                key1: mask & 1 != 0,
                key2: mask & 2 != 0,
                key3: mask & 4 != 0,
                key4: mask & 8 != 0,
            })))
        }

        // OEM Initialization / Handshake packets
        0xf7 | 0xf8 | 0xf9 => Ok(Some(TabletReport::Raw(data.to_vec()))),

        _ => Err(ProtocolError::UnsupportedReportId(report_id)),
    }
}

fn normalize(value: u16, native_max: u16, output_max: u16) -> u16 {
    ((u32::from(value.min(native_max)) * u32::from(output_max) + u32::from(native_max) / 2)
        / u32::from(native_max)) as u16
}
