use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{
    AbsoluteAxisType as Abs, AttributeSet, BusType, EventType, InputEvent, InputId, Key,
    RelativeAxisType, UinputAbsSetup,
};
use huion_proto::config::{KeyAction, TabletConfig};
use huion_proto::packet::{
    ExpressKeyPacket, StylusPacket, HS64_MAX_PRESSURE, HS64_MAX_X, HS64_MAX_Y,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    str::FromStr,
};

pub struct VirtualTablet {
    pen_device: VirtualDevice,
    pad_device: VirtualDevice,
    shortcuts: ShortcutState,
}

/// One virtual keyboard owns the union of all held pad actions. This keeps shared
/// modifiers held until their last owner releases and releases keys before modifiers.
#[derive(Default)]
struct ShortcutState {
    held: Vec<Key>,
    owners: BTreeMap<u16, BTreeSet<Vec<u16>>>,
}

fn action_keys(action: &KeyAction) -> Result<Vec<Key>, String> {
    let names = match action {
        KeyAction::Disabled => return Ok(vec![]),
        KeyAction::Key(name) => vec![name.as_str()],
        KeyAction::KeyCombo(names) => names.iter().map(String::as_str).collect(),
        KeyAction::MouseButton(code) if (272..=279).contains(code) => {
            return Ok(vec![Key::new(*code)])
        }
        KeyAction::MouseButton(_) => {
            return Err("Mouse button must be BTN_LEFT through BTN_TASK".into())
        }
    };
    names
        .into_iter()
        .map(|name| {
            let key = Key::from_str(name).map_err(|_| format!("Unknown Linux key: {name}"))?;
            if !name.starts_with("KEY_") || key.code() == 0 || key.code() >= 0x300 {
                return Err(format!("Unsupported keyboard key: {name}"));
            }
            Ok(key)
        })
        .collect()
}

pub fn validate_config(config: &TabletConfig) -> Result<(), String> {
    config.validate()?;
    for action in &config.express_keys {
        action_keys(action)?;
    }
    if config.target_output.is_some() || config.aspect_ratio_locked {
        return Err("Output mapping and aspect ratio must be configured in your compositor".into());
    }
    Ok(())
}

fn modifier(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTSHIFT
            | Key::KEY_RIGHTSHIFT
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA
    )
}

impl ShortcutState {
    fn update(
        &mut self,
        packet: ExpressKeyPacket,
        config: &TabletConfig,
    ) -> io::Result<Vec<InputEvent>> {
        let mut desired = BTreeSet::new();
        let mut owners: BTreeMap<u16, BTreeSet<Vec<u16>>> = BTreeMap::new();
        for (pressed, action) in [packet.key1, packet.key2, packet.key3, packet.key4]
            .into_iter()
            .zip(&config.express_keys)
        {
            if pressed {
                let keys = action_keys(action)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
                let mut chord: Vec<_> = keys.iter().map(|key| key.code()).collect();
                chord.sort_unstable();
                for key in keys {
                    desired.insert(key.code());
                    if !modifier(key) {
                        owners.entry(key.code()).or_default().insert(chord.clone());
                    }
                }
            }
        }
        let mut next: Vec<Key> = desired.into_iter().map(Key::new).collect();
        next.sort_by_key(|k| (!modifier(*k), k.code()));
        let mut events = Vec::new();
        // Retrigger a shared key only when its owning chord changes (Undo ->
        // Redo). Unrelated held keys/mouse drags must survive other modifiers.
        let retrigger =
            |key: &Key| !modifier(*key) && self.owners.get(&key.code()) != owners.get(&key.code());
        for key in self.held.iter().rev() {
            if !next.contains(key) || retrigger(key) {
                events.push(InputEvent::new(EventType::KEY, key.code(), 0));
            }
        }
        for key in &next {
            if !self.held.contains(key) || retrigger(key) {
                events.push(InputEvent::new(EventType::KEY, key.code(), 1));
            }
        }
        self.held = next;
        self.owners = owners;
        Ok(events)
    }
}

fn stylus_events(packet: StylusPacket, config: &TabletConfig) -> Vec<InputEvent> {
    let (x, y) = config.transform_coordinates(packet.x, packet.y);
    let pressure = if packet.in_range && packet.touch {
        config
            .pressure_curve
            .map_pressure(packet.pressure, HS64_MAX_PRESSURE)
    } else {
        0
    };
    // Touch is first, as required for evdev event ordering. No explicit SYN:
    // VirtualDevice::emit already appends one SYN_REPORT per frame.
    vec![
        InputEvent::new(
            EventType::KEY,
            Key::BTN_TOUCH.code(),
            (packet.in_range && packet.touch) as i32,
        ),
        InputEvent::new(
            EventType::KEY,
            Key::BTN_TOOL_PEN.code(),
            packet.in_range as i32,
        ),
        InputEvent::new(
            EventType::KEY,
            Key::BTN_STYLUS.code(),
            (packet.in_range && packet.barrel_switch) as i32,
        ),
        InputEvent::new(
            EventType::KEY,
            Key::BTN_STYLUS2.code(),
            (packet.in_range && packet.eraser) as i32,
        ),
        InputEvent::new(EventType::ABSOLUTE, Abs::ABS_X.0, x as i32),
        InputEvent::new(EventType::ABSOLUTE, Abs::ABS_Y.0, y as i32),
        InputEvent::new(EventType::ABSOLUTE, Abs::ABS_PRESSURE.0, pressure as i32),
    ]
}

impl VirtualTablet {
    pub fn new() -> io::Result<Self> {
        let axis = |kind, max, resolution| {
            UinputAbsSetup::new(kind, evdev::AbsInfo::new(0, 0, max, 0, 0, resolution))
        };
        let mut pen_keys = AttributeSet::<Key>::new();
        for key in [
            Key::BTN_TOOL_PEN,
            Key::BTN_TOUCH,
            Key::BTN_STYLUS,
            Key::BTN_STYLUS2,
        ] {
            pen_keys.insert(key);
        }
        // BUS_VIRTUAL and no DIRECT property: HS64 is an external pen tablet,
        // not a screen digitizer. Do not masquerade as another Huion USB model.
        let pen_device = VirtualDeviceBuilder::new()?
            .name("Huion HS64 Wayland Stylus")
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x256c, 0x006f, 2))
            .with_keys(&pen_keys)?
            .with_absolute_axis(&axis(Abs::ABS_X, HS64_MAX_X as i32, 250))?
            .with_absolute_axis(&axis(Abs::ABS_Y, HS64_MAX_Y as i32, 245))?
            .with_absolute_axis(&axis(Abs::ABS_PRESSURE, HS64_MAX_PRESSURE as i32, 0))?
            .build()?;
        let mut pad_keys = AttributeSet::<Key>::new();
        // Advertise configurable keyboard and mouse actions, without BTN_0..3
        // joystick codes that can cause duplicate actions in tablet-aware apps.
        for code in 1..0x300 {
            let key = Key::new(code);
            if format!("{key:?}").starts_with("KEY_") {
                pad_keys.insert(key);
            }
        }
        for code in 272..=279 {
            pad_keys.insert(Key::new(code));
        }
        let mut relative_axes = AttributeSet::<RelativeAxisType>::new();
        relative_axes.insert(RelativeAxisType::REL_X);
        relative_axes.insert(RelativeAxisType::REL_Y);
        let pad_device = VirtualDeviceBuilder::new()?
            .name("Huion HS64 Wayland Shortcuts")
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x256c, 0x006f, 2))
            .with_keys(&pad_keys)?
            .with_relative_axes(&relative_axes)?
            .build()?;
        Ok(Self {
            pen_device,
            pad_device,
            shortcuts: ShortcutState::default(),
        })
    }
    pub fn emit_stylus(&mut self, packet: StylusPacket, config: &TabletConfig) -> io::Result<()> {
        self.pen_device.emit(&stylus_events(packet, config))
    }
    pub fn emit_express_keys(
        &mut self,
        packet: ExpressKeyPacket,
        config: &TabletConfig,
    ) -> io::Result<()> {
        let events = self.shortcuts.update(packet, config)?;
        if !events.is_empty() {
            self.pad_device.emit(&events)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(mask: u8) -> ExpressKeyPacket {
        ExpressKeyPacket {
            raw_mask: mask,
            key1: mask & 1 != 0,
            key2: mask & 2 != 0,
            key3: mask & 4 != 0,
            key4: mask & 8 != 0,
        }
    }
    fn tuples(events: Vec<InputEvent>) -> Vec<(u16, i32)> {
        events.iter().map(|e| (e.code(), e.value())).collect()
    }
    #[test]
    fn shortcut_edges_order_and_shared_modifiers() {
        let cfg = TabletConfig::default();
        let mut state = ShortcutState::default();
        assert_eq!(
            tuples(state.update(packet(1), &cfg).unwrap()),
            vec![(29, 1), (44, 1)]
        );
        assert!(state.update(packet(1), &cfg).unwrap().is_empty());
        assert_eq!(
            tuples(state.update(packet(2), &cfg).unwrap()),
            vec![(44, 0), (42, 1), (44, 1)]
        );
        assert_eq!(
            tuples(state.update(packet(0), &cfg).unwrap()),
            vec![(44, 0), (42, 0), (29, 0)]
        );
    }
    #[test]
    fn configured_disabled_and_custom_keys_work() {
        let mut cfg = TabletConfig::default();
        let mut state = ShortcutState::default();
        cfg.express_keys[0] = KeyAction::Disabled;
        assert!(state.update(packet(1), &cfg).unwrap().is_empty());
        cfg.express_keys[0] = KeyAction::Key("KEY_SPACE".into());
        assert_eq!(
            tuples(state.update(packet(1), &cfg).unwrap()),
            vec![(57, 1)]
        );
        cfg.express_keys[0] = KeyAction::Key("KEY_DOES_NOT_EXIST".into());
        assert!(validate_config(&cfg).is_err());
    }
    #[test]
    fn unrelated_shortcuts_do_not_split_mouse_drags_or_repress_pan() {
        for action in [
            KeyAction::MouseButton(272),
            KeyAction::Key("KEY_SPACE".into()),
        ] {
            let mut cfg = TabletConfig::default();
            cfg.express_keys[0] = action;
            cfg.express_keys[1] = KeyAction::KeyCombo(vec!["KEY_LEFTCTRL".into(), "KEY_Z".into()]);
            let mut state = ShortcutState::default();
            state.update(packet(1), &cfg).unwrap();
            assert_eq!(
                tuples(state.update(packet(3), &cfg).unwrap()),
                vec![(29, 1), (44, 1)]
            );
            assert_eq!(
                tuples(state.update(packet(1), &cfg).unwrap()),
                vec![(44, 0), (29, 0)]
            );
        }
    }

    #[test]
    fn hover_and_proximity_out_clear_pressure_and_buttons() {
        let cfg = TabletConfig::default();
        let mut p = StylusPacket {
            x: 100,
            y: 200,
            pressure: HS64_MAX_PRESSURE,
            touch: true,
            barrel_switch: true,
            eraser: false,
            in_range: true,
            tilt_x: 0,
            tilt_y: 0,
        };
        assert!(stylus_events(p, &cfg)
            .iter()
            .any(|e| e.code() == Abs::ABS_PRESSURE.0 && e.value() == HS64_MAX_PRESSURE as i32));
        p.in_range = false;
        assert!(stylus_events(p, &cfg)
            .iter()
            .filter(|e| e.event_type() == EventType::KEY || e.code() == Abs::ABS_PRESSURE.0)
            .all(|e| e.value() == 0));
    }
}

#[cfg(test)]
mod kernel_tests {
    use super::*;
    use huion_proto::packet::{parse_report, TabletReport};
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};

    fn reader(device: &mut VirtualDevice) -> evdev::Device {
        let path = device
            .enumerate_dev_nodes_blocking()
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        // udev assigns evdev permissions asynchronously after UI_DEV_CREATE.
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut reader = loop {
            match evdev::Device::open(&path) {
                Ok(reader) => break reader,
                Err(e)
                    if e.kind() == io::ErrorKind::PermissionDenied && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(e) => panic!("Cannot open {}: {e}", path.display()),
            }
        };
        nix::fcntl::fcntl(
            reader.as_raw_fd(),
            nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
        )
        .unwrap();
        // Keep synthetic regression input away from the desktop.
        reader.grab().unwrap();
        reader
    }
    fn collect(reader: &mut evdev::Device) -> Vec<InputEvent> {
        let end = Instant::now() + Duration::from_secs(1);
        let mut result = Vec::new();
        while Instant::now() < end {
            match reader.fetch_events() {
                Ok(events) => {
                    result.extend(events);
                    if result
                        .iter()
                        .any(|e| e.event_type() == EventType::SYNCHRONIZATION)
                    {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(e) => panic!("evdev read failed: {e}"),
            }
        }
        assert!(!result.is_empty(), "No events reached kernel evdev");
        result
    }

    #[test]
    #[ignore = "requires read/write /dev/uinput and access to resulting evdev nodes"]
    fn raw_reports_reach_kernel_with_pressure_and_configured_shortcuts() {
        let cfg = TabletConfig::default();
        let mut tablet = VirtualTablet::new().unwrap();
        let mut pen = reader(&mut tablet.pen_device);
        let mut pad = reader(&mut tablet.pad_device);
        // Native standard-mode half pressure must become roughly half the
        // canonical range, never a mouse button or a constant-pressure stroke.
        let Some(TabletReport::Stylus(p)) =
            parse_report(&[0x0a, 0xc1, 0x80, 0x3e, 0xd8, 0x27, 0x00, 0x10]).unwrap()
        else {
            panic!()
        };
        tablet.emit_stylus(p, &cfg).unwrap();
        let events = collect(&mut pen);
        assert!(events.iter().any(|e| e.event_type() == EventType::ABSOLUTE
            && e.code() == Abs::ABS_PRESSURE.0
            && (4090..=4105).contains(&e.value())));
        assert_eq!(
            events
                .iter()
                .filter(|e| e.event_type() == EventType::SYNCHRONIZATION)
                .count(),
            1
        );
        let Some(TabletReport::Stylus(p)) = parse_report(&[0x0a, 0x80, 0, 0, 0, 0, 0, 0]).unwrap()
        else {
            panic!()
        };
        tablet.emit_stylus(p, &cfg).unwrap();
        let events = collect(&mut pen);
        assert!(events.iter().any(|e| e.event_type() == EventType::KEY
            && e.code() == Key::BTN_TOOL_PEN.code()
            && e.value() == 0));
        assert!(events.iter().any(|e| e.event_type() == EventType::ABSOLUTE
            && e.code() == Abs::ABS_PRESSURE.0
            && e.value() == 0));
        let Some(TabletReport::ExpressKeys(p)) =
            parse_report(&[3, 5, 0x1d, 0, 0, 0, 0, 0]).unwrap()
        else {
            panic!()
        };
        tablet.emit_express_keys(p, &cfg).unwrap();
        let events = collect(&mut pad);
        let keys: Vec<_> = events
            .iter()
            .filter(|e| e.event_type() == EventType::KEY)
            .map(|e| (e.code(), e.value()))
            .collect();
        assert_eq!(keys, vec![(29, 1), (44, 1)]);
        let Some(TabletReport::ExpressKeys(p)) = parse_report(&[3, 0, 0, 0, 0, 0, 0, 0]).unwrap()
        else {
            panic!()
        };
        tablet.emit_express_keys(p, &cfg).unwrap();
        let events = collect(&mut pad);
        let keys: Vec<_> = events
            .iter()
            .filter(|e| e.event_type() == EventType::KEY)
            .map(|e| (e.code(), e.value()))
            .collect();
        assert_eq!(keys, vec![(44, 0), (29, 0)]);
    }
}
