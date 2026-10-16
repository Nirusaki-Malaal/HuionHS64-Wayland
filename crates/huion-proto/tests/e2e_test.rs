use huion_proto::bezier::PressureCurve;
use huion_proto::config::{ActiveArea, KeyAction, Orientation, TabletConfig};
use huion_proto::packet::{parse_report, TabletReport, HS64_MAX_PRESSURE, HS64_MAX_X, HS64_MAX_Y};

#[test]
fn test_e2e_stylus_drawing_stroke_pipeline() {
    let config = TabletConfig {
        orientation: Orientation::Normal,
        // Set active area to top-left quadrant (20000 x 12500).
        active_area: ActiveArea {
            x: 0,
            y: 0,
            width: 20000,
            height: 12500,
        },
        // Custom Bezier curve: gentle start, steep ramp.
        pressure_curve: PressureCurve::new(0.4, 0.1, 0.7, 0.9),
        ..TabletConfig::default()
    };

    // Simulate drawing stroke of 50 samples
    for step in 0..50 {
        let raw_x = (step * 400) as u16; // 0..19600
        let raw_y = (step * 250) as u16; // 0..12250
        let raw_pressure = (step * 160) as u16; // 0..7840

        let x_le = raw_x.to_le_bytes();
        let y_le = raw_y.to_le_bytes();
        let p_le = raw_pressure.to_le_bytes();

        // Construct raw hardware packet: Report ID 0x09, Touch | InRange = 0x11
        let packet_bytes = [
            0x09, 0x11, x_le[0], x_le[1], y_le[0], y_le[1], p_le[0], p_le[1], 0x00, 0x00, 0x00,
            0x00,
        ];

        let parsed = parse_report(&packet_bytes).expect("parser should succeed");
        assert!(parsed.is_some());

        match parsed.unwrap() {
            TabletReport::Stylus(stylus) => {
                assert_eq!(stylus.x, raw_x);
                assert_eq!(stylus.y, raw_y);
                assert_eq!(stylus.pressure, raw_pressure);
                assert!(stylus.touch);
                assert!(stylus.in_range);

                // Run through coordinate transformation pipeline
                let (mapped_x, mapped_y) = config.transform_coordinates(stylus.x, stylus.y);

                // Active area was half size, so mapped coordinate should scale to full 40000x25000 space
                let expected_mapped_x = ((raw_x as u32 * HS64_MAX_X as u32) / 20000) as u16;
                let expected_mapped_y = ((raw_y as u32 * HS64_MAX_Y as u32) / 12500) as u16;
                assert_eq!(mapped_x, expected_mapped_x);
                assert_eq!(mapped_y, expected_mapped_y);

                // Run through pressure curve calibration pipeline
                let calibrated_p = config
                    .pressure_curve
                    .map_pressure(stylus.pressure, HS64_MAX_PRESSURE);
                assert!(calibrated_p <= HS64_MAX_PRESSURE);
                if raw_pressure > 0 {
                    assert!(calibrated_p > 0);
                }
            }
            _ => panic!("Expected Stylus report"),
        }
    }
}

#[test]
fn test_e2e_express_keys_combos() {
    // Test all 16 button combinations (2^4)
    for mask in 0u8..16 {
        let mut packet_bytes = [0x03, 0, 0, 0, 0, 0, 0, 0];
        for (index, usage) in [0x1d, 0x2c, 0x30, 0x2f].iter().enumerate() {
            if mask & (1 << index) != 0 {
                packet_bytes[2 + index] = *usage;
            }
        }
        let parsed = parse_report(&packet_bytes).expect("parser should succeed");
        assert!(parsed.is_some());

        match parsed.unwrap() {
            TabletReport::ExpressKeys(k) => {
                assert_eq!(k.raw_mask, mask);
                assert_eq!(k.key1, (mask & 0x01) != 0);
                assert_eq!(k.key2, (mask & 0x02) != 0);
                assert_eq!(k.key3, (mask & 0x04) != 0);
                assert_eq!(k.key4, (mask & 0x08) != 0);
            }
            _ => panic!("Expected ExpressKeys report"),
        }
    }
}

#[test]
fn test_e2e_left_handed_orientation_inversion() {
    let config = TabletConfig {
        orientation: Orientation::Rotate180,
        ..TabletConfig::default()
    };

    // Origin (0,0) in 180 deg mode should become max extent (HS64_MAX_X, HS64_MAX_Y)
    let (tx, ty) = config.transform_coordinates(0, 0);
    assert_eq!(tx, HS64_MAX_X);
    assert_eq!(ty, HS64_MAX_Y);

    // Max extent in 180 deg mode should become origin (0, 0)
    let (tx_max, ty_max) = config.transform_coordinates(HS64_MAX_X, HS64_MAX_Y);
    assert_eq!(tx_max, 0);
    assert_eq!(ty_max, 0);
}

#[test]
fn test_e2e_config_serialization_roundtrip() {
    let mut config = TabletConfig {
        device_name: "Huion HS64 Test Edition".to_string(),
        orientation: Orientation::Rotate270,
        pressure_curve: PressureCurve::new(0.1, 0.2, 0.8, 0.9),
        ..TabletConfig::default()
    };
    config.express_keys[0] =
        KeyAction::KeyCombo(vec!["KEY_LEFTCTRL".to_string(), "KEY_S".to_string()]); // Save

    let toml_str = toml::to_string_pretty(&config).expect("should serialize to toml");
    let deserialized: TabletConfig =
        toml::from_str(&toml_str).expect("should deserialize from toml");

    assert_eq!(config, deserialized);
}
