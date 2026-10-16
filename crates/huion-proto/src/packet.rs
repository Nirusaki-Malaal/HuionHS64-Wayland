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
            let in_range = (status & 0x10) != 0 || touch || barrel_switch;

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

        // Express Key Packet
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

        // OEM Initialization / Handshake packets
        0xf7 | 0xf8 | 0xf9 => Ok(Some(TabletReport::Raw(data.to_vec()))),

        _ => Err(ProtocolError::UnsupportedReportId(report_id)),
    }
}
