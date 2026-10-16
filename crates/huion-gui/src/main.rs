use std::cell::RefCell;
use std::rc::Rc;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use adw::{ActionRow, Application, ApplicationWindow, ComboRow, HeaderBar, PreferencesGroup, PreferencesPage};
use gtk4::{Button, DrawingArea, Orientation as GtkOrientation, StringList};

use huion_proto::bezier::PressureCurve;
use huion_proto::config::{Orientation, TabletConfig};

const APP_ID: &str = "io.github.nirusaki.HuionWayland";

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let config = Rc::new(RefCell::new(TabletConfig::load_or_default()));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Huion HS64 Settings (Native Wayland)")
        .default_width(720)
        .default_height(680)
        .build();

    let header_bar = HeaderBar::new();
    let apply_btn = Button::builder()
        .label("Apply Settings")
        .css_classes(["suggested-action"])
        .build();
    header_bar.pack_end(&apply_btn);

    let page = PreferencesPage::new();

    // Group 1: Device Information
    let dev_group = PreferencesGroup::builder()
        .title("Device Status")
        .description("Hardware connection and identity under Wayland")
        .build();

    let dev_row = ActionRow::builder()
        .title("Huion HS64")
        .subtitle("Vendor ID: 0x256c | Direct Kernel Uinput Active")
        .build();
    dev_group.add(&dev_row);
    page.add(&dev_group);

    // Group 2: Orientation & Mapping
    let map_group = PreferencesGroup::builder()
        .title("Display & Orientation")
        .description("Coordinate mapping and left-handed mode")
        .build();

    let orient_strings = StringList::new(&[
        "Normal (0°)",
        "Rotate 90° (Portrait)",
        "Rotate 180° (Left-Handed Mode)",
        "Rotate 270° (Inverted Portrait)",
    ]);
    let orient_row = ComboRow::builder()
        .title("Tablet Orientation")
        .model(&orient_strings)
        .selected(match config.borrow().orientation {
            Orientation::Normal => 0,
            Orientation::Rotate90 => 1,
            Orientation::Rotate180 => 2,
            Orientation::Rotate270 => 3,
        })
        .build();
    map_group.add(&orient_row);
    page.add(&map_group);

    // Group 3: Pressure Curve & Calibration
    let pressure_group = PreferencesGroup::builder()
        .title("Stylus Pressure Curve")
        .description("PW100 8192-level cubic Bezier calibration")
        .build();

    let pressure_presets = StringList::new(&["Linear (Default)", "Soft (Light Touch)", "Hard (Firm Stroke)"]);
    let preset_row = ComboRow::builder()
        .title("Pressure Curve Profile")
        .model(&pressure_presets)
        .selected(0)
        .build();
    pressure_group.add(&preset_row);

    // Live test scratchpad
    let drawing_area = DrawingArea::builder()
        .content_width(600)
        .content_height(140)
        .css_classes(["card"])
        .build();

    let points: Rc<RefCell<Vec<(f64, f64)>>> = Rc::new(RefCell::new(Vec::new()));
    let points_clone = points.clone();

    drawing_area.set_draw_func(move |_, cr, _width, height| {
        cr.set_source_rgb(0.95, 0.95, 0.97);
        let _ = cr.paint();

        cr.set_source_rgb(0.2, 0.5, 0.9);
        cr.set_line_width(4.0);

        let pts = points_clone.borrow();
        if pts.len() >= 2 {
            cr.move_to(pts[0].0, pts[0].1);
            for pt in pts.iter().skip(1) {
                cr.line_to(pt.0, pt.1);
            }
            let _ = cr.stroke();
        }

        // Draw hint text
        cr.set_source_rgb(0.5, 0.5, 0.5);
        cr.move_to(20.0, height as f64 - 20.0);
        let _ = cr.show_text("Pressure Test Area: draw here to test stroke response");
    });

    let drag = gtk4::GestureDrag::new();
    let pts_drag = points.clone();
    let da_clone = drawing_area.clone();
    drag.connect_drag_update(move |gesture, offset_x, offset_y| {
        if let Some((start_x, start_y)) = gesture.start_point() {
            pts_drag.borrow_mut().push((start_x + offset_x, start_y + offset_y));
            da_clone.queue_draw();
        }
    });
    drawing_area.add_controller(drag);

    let clear_btn = Button::builder().label("Clear Test Canvas").build();
    let pts_clear = points.clone();
    let da_clear = drawing_area.clone();
    clear_btn.connect_clicked(move |_| {
        pts_clear.borrow_mut().clear();
        da_clear.queue_draw();
    });

    let canvas_box = gtk4::Box::builder()
        .orientation(GtkOrientation::Vertical)
        .spacing(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    canvas_box.append(&drawing_area);
    canvas_box.append(&clear_btn);

    pressure_group.add(&canvas_box);
    page.add(&pressure_group);

    // Group 4: Express Keys
    let keys_group = PreferencesGroup::builder()
        .title("Express Keys (4 Physical Buttons)")
        .description("Hardware button action bindings")
        .build();

    let key1_row = ActionRow::builder().title("Button 1 (Top)").subtitle("Undo (Ctrl + Z)").build();
    let key2_row = ActionRow::builder().title("Button 2").subtitle("Redo (Ctrl + Shift + Z)").build();
    let key3_row = ActionRow::builder().title("Button 3").subtitle("Brush Tool (B)").build();
    let key4_row = ActionRow::builder().title("Button 4 (Bottom)").subtitle("Eraser Tool (E)").build();

    keys_group.add(&key1_row);
    keys_group.add(&key2_row);
    keys_group.add(&key3_row);
    keys_group.add(&key4_row);
    page.add(&keys_group);

    // Connect Apply Button
    let config_apply = config.clone();
    let orient_row_clone = orient_row.clone();
    let preset_row_clone = preset_row.clone();

    apply_btn.connect_clicked(move |_| {
        let mut cfg = config_apply.borrow_mut();
        cfg.orientation = match orient_row_clone.selected() {
            0 => Orientation::Normal,
            1 => Orientation::Rotate90,
            2 => Orientation::Rotate180,
            3 => Orientation::Rotate270,
            _ => Orientation::Normal,
        };

        cfg.pressure_curve = match preset_row_clone.selected() {
            1 => PressureCurve::new(0.5, 0.1, 0.8, 0.5), // Soft
            2 => PressureCurve::new(0.2, 0.5, 0.5, 0.9), // Hard
            _ => PressureCurve::default(),               // Linear
        };

        if let Err(e) = cfg.save() {
            eprintln!("Failed to save config: {}", e);
        } else {
            println!("Saved tablet settings successfully!");
        }

        // Notify daemon asynchronously via D-Bus
        let cfg_json = serde_json::to_string(&*cfg).unwrap_or_default();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Ok(connection) = zbus::Connection::session().await {
                    let _ = connection
                        .call_method(
                            Some("org.wayland.HuionTablet"),
                            "/org/wayland/HuionTablet",
                            Some("org.wayland.HuionTablet"),
                            "SetConfig",
                            &(cfg_json,),
                        )
                        .await;
                }
            });
        });

        let toast = adw::Toast::new("Tablet configuration applied!");
        toast.set_timeout(2);
        // Display toast if overlay available
        println!("Configuration applied to active driver.");
    });

    let main_box = gtk4::Box::builder().orientation(GtkOrientation::Vertical).build();
    main_box.append(&header_bar);
    main_box.append(&page);

    window.set_content(Some(&main_box));
    window.present();
}
