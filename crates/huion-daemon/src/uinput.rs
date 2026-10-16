use std::io;
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::{
    AbsoluteAxisType, AttributeSet, BusType, EventType, InputEvent,
    InputId, Key, PropType, UinputAbsSetup,
};
use huion_proto::config::TabletConfig;
use huion_proto::packet::{ExpressKeyPacket, StylusPacket, HS64_MAX_PRESSURE, HS64_MAX_X, HS64_MAX_Y};

pub struct VirtualTablet {
    pen_device: VirtualDevice,
    pad_device: VirtualDevice,
}

impl VirtualTablet {
    pub fn new() -> io::Result<Self> {
        // Setup pen absolute axes
        let abs_x = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_X,
            evdev::AbsInfo::new(0, 0, HS64_MAX_X as i32, 0, 0, 200),
        );
        let abs_y = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_Y,
            evdev::AbsInfo::new(0, 0, HS64_MAX_Y as i32, 0, 0, 200),
        );
        let abs_pressure = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_PRESSURE,
            evdev::AbsInfo::new(0, 0, HS64_MAX_PRESSURE as i32, 0, 0, 0),
        );
        let abs_tilt_x = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_TILT_X,
            evdev::AbsInfo::new(0, -60, 60, 0, 0, 0),
        );
        let abs_tilt_y = UinputAbsSetup::new(
            AbsoluteAxisType::ABS_TILT_Y,
            evdev::AbsInfo::new(0, -60, 60, 0, 0, 0),
        );

        let mut pen_keys = AttributeSet::<Key>::new();
        pen_keys.insert(Key::BTN_TOOL_PEN);
        pen_keys.insert(Key::BTN_TOUCH);
        pen_keys.insert(Key::BTN_STYLUS);
        pen_keys.insert(Key::BTN_STYLUS2);

        let mut pen_props = AttributeSet::<PropType>::new();
        pen_props.insert(PropType::DIRECT);

        let pen_device = VirtualDeviceBuilder::new()?
            .name("Huion HS64 Wayland Stylus")
            .input_id(InputId::new(BusType::BUS_USB, 0x256c, 0x006d, 1))
            .with_keys(&pen_keys)?
            .with_absolute_axis(&abs_x)?
            .with_absolute_axis(&abs_y)?
            .with_absolute_axis(&abs_pressure)?
            .with_absolute_axis(&abs_tilt_x)?
            .with_absolute_axis(&abs_tilt_y)?
            .with_properties(&pen_props)?
            .build()?;

        // Setup Pad device (4 express keys + desktop shortcuts)
        let mut pad_keys = AttributeSet::<Key>::new();
        pad_keys.insert(Key::BTN_0);
        pad_keys.insert(Key::BTN_1);
        pad_keys.insert(Key::BTN_2);
        pad_keys.insert(Key::BTN_3);
        pad_keys.insert(Key::KEY_LEFTCTRL);
        pad_keys.insert(Key::KEY_LEFTSHIFT);
        pad_keys.insert(Key::KEY_Z);
        pad_keys.insert(Key::KEY_B);
        pad_keys.insert(Key::KEY_E);

        let pad_device = VirtualDeviceBuilder::new()?
            .name("Huion HS64 Wayland Pad")
            .input_id(InputId::new(BusType::BUS_USB, 0x256c, 0x006d, 1))
            .with_keys(&pad_keys)?
            .build()?;

        Ok(Self {
            pen_device,
            pad_device,
        })
    }

    pub fn emit_stylus(&mut self, packet: StylusPacket, config: &TabletConfig) -> io::Result<()> {
        let (x, y) = config.transform_coordinates(packet.x, packet.y);
        let pressure = config.pressure_curve.map_pressure(packet.pressure, HS64_MAX_PRESSURE);

        let events = [
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, x as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, y as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_PRESSURE.0, pressure as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_TILT_X.0, packet.tilt_x as i32),
            InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_TILT_Y.0, packet.tilt_y as i32),
            InputEvent::new(EventType::KEY, Key::BTN_TOOL_PEN.code(), if packet.in_range { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_TOUCH.code(), if packet.touch { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_STYLUS.code(), if packet.barrel_switch { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_STYLUS2.code(), if packet.eraser { 1 } else { 0 }),
            InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
        ];

        self.pen_device.emit(&events)
    }

    pub fn emit_express_keys(&mut self, packet: ExpressKeyPacket) -> io::Result<()> {
        let events = [
            InputEvent::new(EventType::KEY, Key::BTN_0.code(), if packet.key1 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_1.code(), if packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_2.code(), if packet.key3 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::BTN_3.code(), if packet.key4 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), if packet.key1 || packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_LEFTSHIFT.code(), if packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_Z.code(), if packet.key1 || packet.key2 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_B.code(), if packet.key3 { 1 } else { 0 }),
            InputEvent::new(EventType::KEY, Key::KEY_E.code(), if packet.key4 { 1 } else { 0 }),
            InputEvent::new(EventType::SYNCHRONIZATION, 0, 0),
        ];

        self.pad_device.emit(&events)
    }
}
