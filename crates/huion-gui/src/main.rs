mod preview;
mod tablet_layout;

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc;
use std::time::Duration;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use huion_proto::bezier::PressureCurve;
use huion_proto::config::{KeyAction, Orientation, TabletConfig};

const APP_ID: &str = "io.github.nirusaki.HuionWayland";
const BUS_NAME: &str = "org.wayland.HuionTablet";
const BUS_PATH: &str = "/org/wayland/HuionTablet";

fn main() {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
        } else {
            build_ui(app);
        }
    });
    app.run();
}

fn soft_curve() -> PressureCurve {
    PressureCurve::new(0.2, 0.5, 0.5, 0.9)
}

fn firm_curve() -> PressureCurve {
    PressureCurve::new(0.5, 0.1, 0.8, 0.5)
}

fn curve_selection(curve: PressureCurve) -> u32 {
    if curve == PressureCurve::default() {
        0
    } else if curve == soft_curve() {
        1
    } else if curve == firm_curve() {
        2
    } else {
        3
    }
}

fn action_text(action: &KeyAction) -> String {
    match action {
        KeyAction::Disabled => String::new(),
        KeyAction::Key(key) => key.clone(),
        KeyAction::KeyCombo(keys) => keys.join(" + "),
        KeyAction::MouseButton(code) => match *code {
            272 => "BTN_LEFT".into(),
            273 => "BTN_RIGHT".into(),
            274 => "BTN_MIDDLE".into(),
            275 => "BTN_SIDE".into(),
            276 => "BTN_EXTRA".into(),
            277 => "BTN_FORWARD".into(),
            278 => "BTN_BACK".into(),
            279 => "BTN_TASK".into(),
            _ => format!("BTN_CODE_{code}"),
        },
    }
}

fn parse_action(value: &str) -> Result<KeyAction, String> {
    let value = value.trim().to_ascii_uppercase();
    if value.is_empty() {
        return Ok(KeyAction::Disabled);
    }
    let button = match value.as_str() {
        "BTN_LEFT" => Some(272),
        "BTN_RIGHT" => Some(273),
        "BTN_MIDDLE" => Some(274),
        "BTN_SIDE" => Some(275),
        "BTN_EXTRA" => Some(276),
        "BTN_FORWARD" => Some(277),
        "BTN_BACK" => Some(278),
        "BTN_TASK" => Some(279),
        _ => None,
    };
    if let Some(code) = button {
        return Ok(KeyAction::MouseButton(code));
    }
    let keys: Vec<String> = value.split('+').map(|key| key.trim().to_owned()).collect();
    if keys.iter().any(|key| {
        !key.starts_with("KEY_")
            || key.len() <= 4
            || !key
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }) {
        return Err("Use Linux key names such as KEY_LEFTCTRL + KEY_Z, a BTN_LEFT mouse button, or leave blank to disable.".into());
    }
    for key in &keys {
        let code = evdev::Key::from_str(key)
            .map_err(|_| format!("Unknown Linux key name: {key}"))?
            .code();
        if code == 0 || code >= 0x300 {
            return Err(format!(
                "This key cannot be used as a tablet shortcut: {key}"
            ));
        }
    }
    let mut distinct = std::collections::HashSet::new();
    if keys.iter().any(|key| !distinct.insert(key)) {
        return Err("A shortcut cannot contain the same key twice.".into());
    }
    if keys.len() == 1 {
        Ok(KeyAction::Key(keys[0].clone()))
    } else {
        Ok(KeyAction::KeyCombo(keys))
    }
}

const CUSTOM_SHORTCUT: u32 = 8;
const SHORTCUT_LABELS: &[&str] = &[
    "Undo · Ctrl+Z",
    "Redo · Ctrl+Shift+Z",
    "Brush · B",
    "Eraser · E",
    "Pan · Space",
    "Brush larger · ]",
    "Brush smaller · [",
    "Disabled",
    "Custom…",
];

fn shortcut_preset(index: u32) -> Option<KeyAction> {
    match index {
        0 => Some(KeyAction::KeyCombo(vec![
            "KEY_LEFTCTRL".into(),
            "KEY_Z".into(),
        ])),
        1 => Some(KeyAction::KeyCombo(vec![
            "KEY_LEFTCTRL".into(),
            "KEY_LEFTSHIFT".into(),
            "KEY_Z".into(),
        ])),
        2 => Some(KeyAction::Key("KEY_B".into())),
        3 => Some(KeyAction::Key("KEY_E".into())),
        4 => Some(KeyAction::Key("KEY_SPACE".into())),
        5 => Some(KeyAction::Key("KEY_RIGHTBRACE".into())),
        6 => Some(KeyAction::Key("KEY_LEFTBRACE".into())),
        7 => Some(KeyAction::Disabled),
        _ => None,
    }
}

fn shortcut_selection(action: &KeyAction) -> u32 {
    (0..CUSTOM_SHORTCUT)
        .find(|index| shortcut_preset(*index).as_ref() == Some(action))
        .unwrap_or(CUSTOM_SHORTCUT)
}

fn selected_shortcut(index: u32, custom: &str) -> Result<KeyAction, String> {
    if index == CUSTOM_SHORTCUT {
        parse_action(custom)
    } else {
        shortcut_preset(index).ok_or_else(|| "Choose a tablet button action.".into())
    }
}

fn shortcut_caption(index: u32) -> String {
    SHORTCUT_LABELS
        .get(index as usize)
        .unwrap_or(&"Custom…")
        .split(" · ")
        .next()
        .unwrap_or("Custom…")
        .to_string()
}

struct ShortcutRow {
    preset: adw::ComboRow,
    custom: adw::EntryRow,
}

impl ShortcutRow {
    fn new(group: &adw::PreferencesGroup, index: usize, action: &KeyAction) -> Self {
        let selected = shortcut_selection(action);
        let preset = adw::ComboRow::builder()
            .title(tablet_layout::BUTTON_POSITIONS[index])
            .subtitle(format!("Button {}", index + 1))
            .model(&gtk4::StringList::new(SHORTCUT_LABELS))
            .selected(selected)
            .build();
        let custom = adw::EntryRow::builder()
            .title("Custom · Linux key names")
            .text(action_text(action))
            .visible(selected == CUSTOM_SHORTCUT)
            .build();
        custom.set_tooltip_text(Some("Use KEY_LEFTCTRL + KEY_Z for a keyboard shortcut, BTN_LEFT / BTN_RIGHT / BTN_MIDDLE for a mouse button, or leave blank to disable."));
        let custom_visibility = custom.clone();
        preset.connect_selected_notify(move |row| {
            custom_visibility.set_visible(row.selected() == CUSTOM_SHORTCUT);
            custom_visibility.remove_css_class("error");
        });
        group.add(&preset);
        group.add(&custom);
        Self { preset, custom }
    }
}

enum WorkerMessage {
    Status(Result<bool, String>),
    Applied(Result<String, String>),
}

/// D-Bus and disk operations stay off the GTK main thread.
fn start_worker() -> (mpsc::Sender<TabletConfig>, mpsc::Receiver<WorkerMessage>) {
    let (requests, incoming) = mpsc::channel::<TabletConfig>();
    let (outgoing, results) = mpsc::channel();
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = outgoing.send(WorkerMessage::Status(Err(error.to_string())));
                return;
            }
        };
        loop {
            let status = runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(2), async {
                    let connection = zbus::Connection::session()
                        .await
                        .map_err(|e| e.to_string())?;
                    let reply = connection
                        .call_method(
                            Some(BUS_NAME),
                            BUS_PATH,
                            Some(BUS_NAME),
                            "GetDeviceInfo",
                            &(),
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                    let json: String = reply.body().deserialize().map_err(|e| e.to_string())?;
                    let info: serde_json::Value =
                        serde_json::from_str(&json).map_err(|e| e.to_string())?;
                    Ok(info["connected"].as_bool().unwrap_or(false))
                })
                .await
                .map_err(|_| "Driver did not respond".to_string())?
            });
            if outgoing.send(WorkerMessage::Status(status)).is_err() {
                break;
            }
            match incoming.recv_timeout(Duration::from_secs(3)) {
                Ok(config) => {
                    let result = runtime.block_on(apply_config(config));
                    if outgoing.send(WorkerMessage::Applied(result)).is_err() {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    (requests, results)
}

async fn apply_config(config: TabletConfig) -> Result<String, String> {
    config.validate()?;
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    let live = tokio::time::timeout(Duration::from_secs(3), async {
        let connection = zbus::Connection::session()
            .await
            .map_err(|e| e.to_string())?;
        let reply = connection
            .call_method(
                Some(BUS_NAME),
                BUS_PATH,
                Some(BUS_NAME),
                "SetConfig",
                &(json,),
            )
            .await
            .map_err(|e| e.to_string())?;
        reply
            .body()
            .deserialize::<bool>()
            .map_err(|e| e.to_string())
    })
    .await;
    match live {
        Ok(Ok(true)) => Ok("Settings applied to the driver".into()),
        Ok(Ok(false)) => {
            Err("The driver rejected these settings. Check key names and the driver log.".into())
        }
        _ => {
            config
                .save()
                .map_err(|e| format!("Could not save settings: {e}"))?;
            Ok("Settings saved locally. Start or restart the driver to apply them.".into())
        }
    }
}

/// Old versions exposed mapping switches without implementing them. Keep the
/// user's useful pen/button settings and explicitly reset those stale switches.
fn clear_unsupported_mapping(config: &mut TabletConfig) -> bool {
    let changed = config.aspect_ratio_locked || config.target_output.is_some();
    config.aspect_ratio_locked = false;
    config.target_output = None;
    changed
}

fn decode_preferences(content: &str) -> Result<(TabletConfig, bool), String> {
    let mut config: TabletConfig = toml::from_str(content).map_err(|error| error.to_string())?;
    // Clear even malformed legacy output names before validating useful fields.
    let cleared_mapping = clear_unsupported_mapping(&mut config);
    config.validate()?;
    Ok((config, cleared_mapping))
}

fn settings_page_box() -> gtk4::Box {
    gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(24)
        .margin_top(20)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .vexpand(true)
        .build()
}

fn scrolling_page(child: &impl IsA<gtk4::Widget>) -> gtk4::ScrolledWindow {
    let clamp = adw::Clamp::builder()
        .maximum_size(760)
        .tightening_threshold(520)
        .vexpand(true)
        .child(child)
        .build();
    gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .child(&clamp)
        .build()
}

fn build_ui(app: &adw::Application) {
    let path = TabletConfig::config_path();
    let (initial, load_error) = match std::fs::read_to_string(&path) {
        Ok(content) => match decode_preferences(&content) {
            Ok((config, cleared_mapping)) => (
                config,
                cleared_mapping.then(|| "Saved display overrides are unsupported. Apply to clear them; configure monitor mapping and aspect ratio in your desktop's tablet settings.".into()),
            ),
            Err(error) => (
                TabletConfig::default(),
                Some(format!("Could not read saved settings: {error}")),
            ),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (TabletConfig::default(), None)
        }
        Err(error) => (
            TabletConfig::default(),
            Some(format!("Could not open settings: {error}")),
        ),
    };
    let config = Rc::new(RefCell::new(initial));
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Huion Tablet")
        .default_width(760)
        .default_height(860)
        .build();
    let overlay = adw::ToastOverlay::new();
    let layout = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Huion Tablet",
        "HS64 · Pen & tablet settings",
    )));
    let apply = gtk4::Button::builder()
        .label("Apply")
        .css_classes(["suggested-action"])
        .valign(gtk4::Align::Center)
        .build();
    apply.set_tooltip_text(Some("Save settings and send them to the running driver"));
    header.pack_end(&apply);
    layout.append(&header);

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();
    let clamp = adw::Clamp::builder()
        .maximum_size(760)
        .tightening_threshold(520)
        .build();
    let page = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(24)
        .margin_top(24)
        .margin_bottom(32)
        .margin_start(24)
        .margin_end(24)
        .build();

    let hero = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    let icon = gtk4::Image::from_icon_name("input-tablet-symbolic");
    icon.set_pixel_size(48);
    icon.add_css_class("accent");
    hero.append(&icon);
    let title_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    title_box.append(
        &gtk4::Label::builder()
            .label("Make every stroke yours")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["title-1"])
            .build(),
    );
    title_box.append(
        &gtk4::Label::builder()
            .label("Tune your pen, tablet orientation and shortcuts.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label"])
            .build(),
    );
    hero.append(&title_box);
    page.append(&hero);

    let status_group = adw::PreferencesGroup::new();
    let status = adw::ActionRow::builder()
        .title("Checking driver…")
        .subtitle("Looking for the tablet service on your session bus")
        .build();
    let status_icon = gtk4::Image::from_icon_name("network-offline-symbolic");
    status.add_prefix(&status_icon);
    status_group.add(&status);
    page.append(&status_group);

    let mapping = adw::PreferencesGroup::builder()
        .title("Orientation and display")
        .description("Rotate the tablet here. Configure monitor mapping and aspect ratio in your desktop’s tablet settings.")
        .build();
    let orientation = adw::ComboRow::builder()
        .title("Rotation")
        .model(&gtk4::StringList::new(&[
            "0° · Standard",
            "90° · Clockwise",
            "180° · Left handed",
            "270° · Counterclockwise",
        ]))
        .selected(match config.borrow().orientation {
            Orientation::Normal => 0,
            Orientation::Rotate90 => 1,
            Orientation::Rotate180 => 2,
            Orientation::Rotate270 => 3,
        })
        .build();
    mapping.add(&orientation);
    page.append(&mapping);

    let pressure_group = adw::PreferencesGroup::builder()
        .title("Pen pressure")
        .description("Choose how quickly your stroke reaches full pressure.")
        .build();
    let preset = adw::ComboRow::builder()
        .title("Response")
        .model(&gtk4::StringList::new(&[
            "Linear · Natural response",
            "Soft · Less force",
            "Firm · More control",
            "Custom · Keep saved curve",
        ]))
        .selected(curve_selection(config.borrow().pressure_curve))
        .build();
    pressure_group.add(&preset);
    let curve = gtk4::DrawingArea::builder()
        .content_height(100)
        .margin_top(12)
        .build();
    let preset_curve = preset.clone();
    let custom_curve = config.borrow().pressure_curve;
    curve.set_draw_func(move |area, cr, width, height| {
        let color = area.style_context().color();
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.18,
        );
        cr.set_line_width(1.0);
        let (left, top, w, h) = (
            16.0,
            8.0,
            (width - 32).max(1) as f64,
            (height - 24).max(1) as f64,
        );
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            cr.move_to(left + t * w, top);
            cr.line_to(left + t * w, top + h);
            cr.move_to(left, top + t * h);
            cr.line_to(left + w, top + t * h);
        }
        let _ = cr.stroke();
        let curve = match preset_curve.selected() {
            1 => soft_curve(),
            2 => firm_curve(),
            3 => custom_curve,
            _ => PressureCurve::default(),
        };
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.9,
        );
        cr.set_line_width(2.5);
        for i in 0..=100 {
            let x = i as f64 / 100.0;
            let y = curve.evaluate(x);
            if i == 0 {
                cr.move_to(left, top + h);
            } else {
                cr.line_to(left + x * w, top + (1.0 - y) * h);
            }
        }
        let _ = cr.stroke();
    });
    let curve_redraw = curve.downgrade();
    preset.connect_selected_notify(move |_| {
        if let Some(curve) = curve_redraw.upgrade() {
            curve.queue_draw();
        }
    });
    pressure_group.add(&curve);
    page.append(&pressure_group);

    let test_page = settings_page_box();
    test_page.set_spacing(12);
    test_page.append(&preview::pen_test());
    let buttons_page = settings_page_box();

    let diagram_actions = Rc::new(RefCell::new(std::array::from_fn(|index| {
        shortcut_caption(shortcut_selection(&config.borrow().express_keys[index]))
    })));
    let diagram = tablet_layout::diagram(diagram_actions.clone());
    let diagram_group = adw::PreferencesGroup::builder()
        .title("Find your buttons")
        .description("Cable at top; left and right as pictured. Numbers match the controls below. This reference stays fixed when you rotate your tablet.")
        .build();
    diagram_group.add(&diagram);
    buttons_page.append(&diagram_group);
    let keys_group = adw::PreferencesGroup::builder()
        .title("Assign shortcuts")
        .description("Choose an action for each button, then press Apply. Shortcuts follow your drawing app’s keyboard settings.")
        .build();
    let key_rows: Vec<_> = config
        .borrow()
        .express_keys
        .iter()
        .enumerate()
        .map(|(index, action)| {
            let row = ShortcutRow::new(&keys_group, index, action);
            let actions = diagram_actions.clone();
            let diagram = diagram.downgrade();
            row.preset.connect_selected_notify(move |row| {
                actions.borrow_mut()[index] = shortcut_caption(row.selected());
                if let Some(diagram) = diagram.upgrade() {
                    diagram.queue_draw();
                }
            });
            row
        })
        .collect();
    buttons_page.append(&keys_group);
    page.append(
        &gtk4::Label::builder()
            .label("Settings are stored in your user configuration folder.")
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label", "caption"])
            .build(),
    );

    clamp.set_child(Some(&page));
    scroller.set_child(Some(&clamp));
    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);
    stack.add_titled_with_icon(&scroller, Some("tablet"), "Tablet", "input-tablet-symbolic");
    stack.add_titled_with_icon(
        &scrolling_page(&test_page),
        Some("pen"),
        "Try your pen",
        "document-edit-symbolic",
    );
    stack.add_titled_with_icon(
        &scrolling_page(&buttons_page),
        Some("buttons"),
        "Buttons",
        "input-keyboard-symbolic",
    );
    let navigation = adw::ViewSwitcherBar::new();
    navigation.set_stack(Some(&stack));
    navigation.set_reveal(true);
    layout.append(&stack);
    layout.append(&navigation);
    overlay.set_child(Some(&layout));
    window.set_content(Some(&overlay));
    if let Some(error) = load_error {
        let toast = adw::Toast::new(&error);
        toast.set_timeout(0);
        overlay.add_toast(toast);
    }

    let (requests, results) = start_worker();
    let overlay_apply = overlay.clone();
    let config_apply = config.clone();
    apply.connect_clicked(move |button| {
        let mut candidate = config_apply.borrow().clone();
        clear_unsupported_mapping(&mut candidate);
        candidate.orientation = match orientation.selected() {
            1 => Orientation::Rotate90,
            2 => Orientation::Rotate180,
            3 => Orientation::Rotate270,
            _ => Orientation::Normal,
        };
        candidate.pressure_curve = match preset.selected() {
            1 => soft_curve(),
            2 => firm_curve(),
            3 => custom_curve,
            _ => PressureCurve::default(),
        };
        for (index, row) in key_rows.iter().enumerate() {
            match selected_shortcut(row.preset.selected(), &row.custom.text()) {
                Ok(action) => {
                    candidate.express_keys[index] = action;
                    row.custom.remove_css_class("error");
                }
                Err(error) => {
                    row.custom.add_css_class("error");
                    row.custom.grab_focus();
                    overlay_apply.add_toast(adw::Toast::new(&error));
                    return;
                }
            }
        }
        if requests.send(candidate.clone()).is_ok() {
            *config_apply.borrow_mut() = candidate;
            button.set_sensitive(false);
            button.set_label("Applying…");
        } else {
            overlay_apply.add_toast(adw::Toast::new(
                "The settings worker stopped. Close and reopen this window.",
            ));
        }
    });
    let window_weak = window.downgrade();
    gtk4::glib::timeout_add_local(Duration::from_millis(100), move || {
        if window_weak.upgrade().is_none() {
            return gtk4::glib::ControlFlow::Break;
        }
        while let Ok(message) = results.try_recv() {
            match message {
                WorkerMessage::Status(Ok(true)) => {
                    status.set_tooltip_text(None);
                    status.set_title("Huion HS64 connected");
                    status.set_subtitle("The driver reports an attached tablet");
                    status_icon.set_icon_name(Some("emblem-ok-symbolic"));
                }
                WorkerMessage::Status(Ok(false)) => {
                    status.set_tooltip_text(None);
                    status.set_title("Waiting for your tablet");
                    status.set_subtitle("Driver is running. Connect your Huion HS64 to begin.");
                    status_icon.set_icon_name(Some("input-tablet-symbolic"));
                }
                WorkerMessage::Status(Err(error)) => {
                    status.set_title("Driver unavailable");
                    status.set_subtitle(
                        "You can save settings locally. Start huion-daemon to use your tablet.",
                    );
                    status.set_tooltip_text(Some(&error));
                    status_icon.set_icon_name(Some("network-offline-symbolic"));
                }
                WorkerMessage::Applied(result) => {
                    apply.set_sensitive(true);
                    apply.set_label("Apply");
                    let text = match result {
                        Ok(text) => text,
                        Err(error) => error,
                    };
                    let toast = adw::Toast::new(&text);
                    toast.set_timeout(6);
                    overlay.add_toast(toast);
                }
            }
        }
        gtk4::glib::ControlFlow::Continue
    });
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_and_firm_presets_have_correct_response() {
        for input in [0.1, 0.25, 0.5, 0.75, 0.9] {
            assert!(soft_curve().evaluate(input) > input);
            assert!(firm_curve().evaluate(input) < input);
        }
    }

    #[test]
    fn existing_pressure_curves_select_matching_profile() {
        assert_eq!(curve_selection(PressureCurve::default()), 0);
        assert_eq!(curve_selection(soft_curve()), 1);
        assert_eq!(curve_selection(firm_curve()), 2);
        assert_eq!(curve_selection(PressureCurve::new(0.1, 0.3, 0.8, 0.7)), 3);
    }

    #[test]
    fn key_bindings_roundtrip_without_resetting_saved_actions() {
        let mut actions = TabletConfig::default().express_keys.to_vec();
        actions.extend([
            KeyAction::Disabled,
            KeyAction::MouseButton(272),
            KeyAction::MouseButton(276),
            KeyAction::MouseButton(279),
        ]);
        for action in actions {
            assert_eq!(parse_action(&action_text(&action)).unwrap(), action);
        }
        assert_eq!(
            parse_action(" key_leftctrl + key_a ").unwrap(),
            KeyAction::KeyCombo(vec!["KEY_LEFTCTRL".into(), "KEY_A".into()])
        );
    }

    #[test]
    fn shortcuts_restore_presets_and_preserve_custom_actions() {
        for index in 0..CUSTOM_SHORTCUT {
            let action = shortcut_preset(index).unwrap();
            assert_eq!(shortcut_selection(&action), index);
            assert_eq!(
                selected_shortcut(index, "unused custom text").unwrap(),
                action
            );
        }
        for (index, action) in TabletConfig::default().express_keys.iter().enumerate() {
            assert_eq!(shortcut_selection(action), index as u32);
        }
        for custom in [
            KeyAction::Key("KEY_F15".into()),
            KeyAction::KeyCombo(vec!["KEY_LEFTALT".into(), "KEY_A".into()]),
            KeyAction::MouseButton(272),
            KeyAction::MouseButton(279),
        ] {
            assert_eq!(shortcut_selection(&custom), CUSTOM_SHORTCUT);
            assert_eq!(
                selected_shortcut(CUSTOM_SHORTCUT, &action_text(&custom)).unwrap(),
                custom
            );
        }
        assert!(selected_shortcut(99, "").is_err());
    }

    #[test]
    fn stale_mapping_options_do_not_prevent_applying_pen_settings() {
        let mut config = TabletConfig {
            target_output: Some("DP-1".into()),
            aspect_ratio_locked: true,
            pressure_curve: soft_curve(),
            ..TabletConfig::default()
        };
        let keys = config.express_keys.clone();
        assert!(clear_unsupported_mapping(&mut config));
        assert_eq!(config.target_output, None);
        assert!(!config.aspect_ratio_locked);
        assert_eq!(config.pressure_curve, soft_curve());
        assert_eq!(config.express_keys, keys);
        assert!(!clear_unsupported_mapping(&mut config));
        config.target_output = Some(String::new());
        let serialized = toml::to_string(&config).unwrap();
        let (loaded, cleared) = decode_preferences(&serialized).unwrap();
        assert!(cleared);
        assert_eq!(loaded.pressure_curve, soft_curve());
        assert_eq!(loaded.express_keys, keys);
    }

    #[test]
    fn invalid_shortcuts_fail_instead_of_silently_disabling_buttons() {
        for invalid in [
            "Ctrl + Z",
            "KEY_",
            "KEY_A +",
            "BTN_UNKNOWN",
            "KEY_A + BTN_LEFT",
            "KEY_THIS_DOES_NOT_EXIST",
            "KEY_Z + KEY_Z",
            "KEY_RESERVED",
            "KEY_MAX",
        ] {
            assert!(parse_action(invalid).is_err(), "{invalid}");
        }
    }
}
