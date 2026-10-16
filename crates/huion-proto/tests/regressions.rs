use huion_proto::{
    parse_report, ActiveArea, KeyAction, Orientation, TabletConfig, TabletReport,
    HS64_MAX_PRESSURE, HS64_MAX_X, HS64_MAX_Y,
};

fn standard_pen(status: u8, x: u16, y: u16, pressure: u16) -> huion_proto::StylusPacket {
    let mut report = vec![0x0a, status];
    report.extend(x.to_le_bytes());
    report.extend(y.to_le_bytes());
    report.extend(pressure.to_le_bytes());
    let Some(TabletReport::Stylus(pen)) = parse_report(&report).unwrap() else {
        panic!("expected pen")
    };
    pen
}

#[test]
fn standard_descriptor_ranges_reach_full_canonical_output() {
    let pen = standard_pen(0x41, 32000, 20400, 4095);
    assert_eq!(
        (pen.x, pen.y, pen.pressure),
        (HS64_MAX_X, HS64_MAX_Y, HS64_MAX_PRESSURE)
    );
    assert!(pen.in_range && pen.touch);
    // The prior parser treated native full pressure as half pressure.
    assert!(standard_pen(0x41, 16000, 10200, 2048).pressure > HS64_MAX_PRESSURE / 2);
    let clamped = standard_pen(0x41, u16::MAX, u16::MAX, u16::MAX);
    assert_eq!(
        (clamped.x, clamped.y, clamped.pressure),
        (HS64_MAX_X, HS64_MAX_Y, HS64_MAX_PRESSURE)
    );
}

#[test]
fn standard_proximity_and_pressure_release_follow_descriptor() {
    assert!(!standard_pen(0x10, 0, 0, 1000).in_range);
    let hover = standard_pen(0x40, 0, 0, 1000);
    assert!(hover.in_range);
    assert_eq!(hover.pressure, 0);
    let out = standard_pen(0x01, 0, 0, 1000);
    assert!(!out.in_range && !out.touch);
    assert_eq!(out.pressure, 0);
}

fn key_mask(report: [u8; 8]) -> u8 {
    let Some(TabletReport::ExpressKeys(keys)) = parse_report(&report).unwrap() else {
        panic!("expected keys")
    };
    keys.raw_mask
}

#[test]
fn keyboard_reports_parse_all_six_usages_without_phantom_fourth_button() {
    assert_eq!(key_mask([3, 5, 0x1d, 0, 0x2c, 0x30, 0, 0x2f]), 15);
    assert_eq!(key_mask([3, 5, 0, 0, 0, 0, 0, 0]), 0); // modifiers only
    assert_eq!(key_mask([3, 0, 0x04, 0, 0, 0, 0, 0]), 0); // unknown A
    assert_eq!(key_mask([3, 0, 0, 0, 0, 0, 0, 0]), 0);
    for error in 1..=3 {
        assert_eq!(parse_report(&[3, 0, error, 0, 0, 0, 0, 0]), Ok(None));
    }
    assert!(parse_report(&[3, 0, 0x1d]).is_err());
}

#[test]
fn quarter_turns_map_corners_within_the_advertised_axes() {
    let mut cfg = TabletConfig {
        orientation: Orientation::Rotate90,
        ..TabletConfig::default()
    };
    assert_eq!(cfg.transform_coordinates(0, 0), (HS64_MAX_X, 0));
    assert_eq!(
        cfg.transform_coordinates(HS64_MAX_X, 0),
        (HS64_MAX_X, HS64_MAX_Y)
    );
    assert_eq!(cfg.transform_coordinates(0, HS64_MAX_Y), (0, 0));
    assert_eq!(
        cfg.transform_coordinates(HS64_MAX_X, HS64_MAX_Y),
        (0, HS64_MAX_Y)
    );
    cfg.orientation = Orientation::Rotate270;
    assert_eq!(cfg.transform_coordinates(0, 0), (0, HS64_MAX_Y));
    assert_eq!(
        cfg.transform_coordinates(HS64_MAX_X, HS64_MAX_Y),
        (HS64_MAX_X, 0)
    );
    for orientation in [
        Orientation::Normal,
        Orientation::Rotate90,
        Orientation::Rotate180,
        Orientation::Rotate270,
    ] {
        cfg.orientation = orientation;
        for (x, y) in [(0, 0), (u16::MAX, u16::MAX), (20000, 12500), (40000, 0)] {
            let (x, y) = cfg.transform_coordinates(x, y);
            assert!(x <= HS64_MAX_X && y <= HS64_MAX_Y);
        }
    }
}

#[test]
fn validation_rejects_empty_overflowing_areas_and_invalid_curves() {
    let mut cfg = TabletConfig::default();
    assert!(cfg.validate().is_ok());
    for area in [
        ActiveArea {
            x: 0,
            y: 0,
            width: 0,
            height: 100,
        },
        ActiveArea {
            x: u16::MAX,
            y: 0,
            width: 10,
            height: 10,
        },
        ActiveArea {
            x: 0,
            y: 25000,
            width: 10,
            height: 1,
        },
    ] {
        cfg.active_area = area;
        assert!(cfg.validate().is_err());
    }
    cfg.active_area = ActiveArea::default();
    for invalid in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        cfg.pressure_curve.p1.x = invalid;
        assert!(cfg.validate().is_err());
    }
}

#[test]
fn validation_checks_action_shapes_and_mouse_button_range() {
    let mut cfg = TabletConfig::default();
    for action in [
        KeyAction::Key("".into()),
        KeyAction::Key("key_z".into()),
        KeyAction::KeyCombo(vec![]),
        KeyAction::MouseButton(1),
    ] {
        cfg.express_keys[0] = action;
        assert!(cfg.validate().is_err());
    }
    cfg.express_keys[0] = KeyAction::MouseButton(272);
    assert!(cfg.validate().is_ok());
}
