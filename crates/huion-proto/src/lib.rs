pub mod bezier;
pub mod config;
pub mod packet;

pub use bezier::{BezierPoint, PressureCurve};
pub use config::{ActiveArea, KeyAction, Orientation, TabletConfig};
pub use packet::{
    parse_report, ExpressKeyPacket, ProtocolError, StylusPacket, TabletReport, HS64_MAX_PRESSURE,
    HS64_MAX_X, HS64_MAX_Y,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stylus_hover() {
        // Report ID 0x07, In-range (0x10), X=10000, Y=5000, Pressure=0
        let x_bytes = 10000u16.to_le_bytes();
        let y_bytes = 5000u16.to_le_bytes();
        let p_bytes = 0u16.to_le_bytes();
        let data = vec![0x07, 0x10, x_bytes[0], x_bytes[1], y_bytes[0], y_bytes[1], p_bytes[0], p_bytes[1]];

        let res = parse_report(&data).unwrap().unwrap();
        match res {
            TabletReport::Stylus(s) => {
                assert_eq!(s.x, 10000);
                assert_eq!(s.y, 5000);
                assert_eq!(s.pressure, 0);
                assert!(s.in_range);
                assert!(!s.touch);
                assert!(!s.barrel_switch);
            }
            _ => panic!("expected stylus packet"),
        }
    }

    #[test]
    fn test_parse_stylus_touch_and_barrel() {
        // Report ID 0x09, Touch (0x01) | Barrel Switch (0x02) = 0x03, Pressure=4096
        let x_bytes = 20000u16.to_le_bytes();
        let y_bytes = 12000u16.to_le_bytes();
        let p_bytes = 4096u16.to_le_bytes();
        let data = vec![0x09, 0x03, x_bytes[0], x_bytes[1], y_bytes[0], y_bytes[1], p_bytes[0], p_bytes[1]];

        let res = parse_report(&data).unwrap().unwrap();
        match res {
            TabletReport::Stylus(s) => {
                assert_eq!(s.x, 20000);
                assert_eq!(s.y, 12000);
                assert_eq!(s.pressure, 4096);
                assert!(s.touch);
                assert!(s.barrel_switch);
                assert!(s.in_range);
            }
            _ => panic!("expected stylus packet"),
        }
    }

    #[test]
    fn test_parse_express_keys() {
        // Report ID 0x08, Key 1 and Key 3 pressed (mask: 0x01 | 0x04 = 0x05)
        let data = vec![0x08, 0x05, 0x00, 0x00];
        let res = parse_report(&data).unwrap().unwrap();
        match res {
            TabletReport::ExpressKeys(k) => {
                assert!(k.key1);
                assert!(!k.key2);
                assert!(k.key3);
                assert!(!k.key4);
            }
            _ => panic!("expected express keys packet"),
        }
    }

    #[test]
    fn test_parse_express_keys_report3() {
        // Report ID 0x03: Button 1 (mod=0x05, key=0x1d)
        let btn1_data = vec![0x03, 0x05, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x00];
        let res1 = parse_report(&btn1_data).unwrap().unwrap();
        match res1 {
            TabletReport::ExpressKeys(k) => {
                assert!(k.key1);
                assert!(!k.key2);
                assert!(!k.key3);
                assert!(!k.key4);
            }
            _ => panic!("expected express keys packet"),
        }

        // Release event
        let release_data = vec![0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let res_rel = parse_report(&release_data).unwrap().unwrap();
        match res_rel {
            TabletReport::ExpressKeys(k) => {
                assert!(!k.key1 && !k.key2 && !k.key3 && !k.key4);
            }
            _ => panic!("expected release packet"),
        }
    }

    #[test]
    fn test_bezier_curve_evaluation() {
        let curve = PressureCurve::default();
        // Default linear curve should return ~input
        let p_mid = curve.evaluate(0.5);
        assert!((p_mid - 0.5).abs() < 0.05);

        // Custom curve: slow start (soft pressure)
        let soft_curve = PressureCurve::new(0.5, 0.1, 0.9, 0.5);
        let soft_mid = soft_curve.evaluate(0.5);
        assert!(soft_mid < 0.4);
    }

    #[test]
    fn test_coordinate_transformation() {
        let mut cfg = TabletConfig::default();
        // Normal 0 deg
        assert_eq!(cfg.transform_coordinates(10000, 5000), (10000, 5000));

        // Rotate 180 deg (Left-handed)
        cfg.orientation = Orientation::Rotate180;
        let (rx, ry) = cfg.transform_coordinates(0, 0);
        assert_eq!(rx, HS64_MAX_X);
        assert_eq!(ry, HS64_MAX_Y);
    }
}
