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
        // Stylus Packet (Standard Huion Report ID 0x07, 0x08, or 0x09 depending on interface mode)
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
            // On UC-Logic HS64 digitizers, bit 6 (0x40) or bit 4 (0x10) indicates in-range proximity
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

        // Express Key Packet (Report ID 0x08 bitmask)
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

        // Express Key Packet (Report ID 0x03 USB HID Keyboard reports from HS64 interface 2)
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

            // Map HS64 hardware shortcut scancodes:
            // Button 1 (Top): 0x1d (Z / Undo)
            // Button 2: 0x2c (Space / Hand)
            // Button 3: 0x30 (']' / Brush+)
            // Button 4 (Bottom): any remaining keycode like 0x2f ('[' / Brush-)
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

        // OEM Initialization / Handshake packets
        0xf7 | 0xf8 | 0xf9 => Ok(Some(TabletReport::Raw(data.to_vec()))),

        _ => Err(ProtocolError::UnsupportedReportId(report_id)),
    }
}
