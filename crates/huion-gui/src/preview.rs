//! Local drawing workspace. Canvas rotation never changes the driver's mapping.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone, Copy, Debug)]
struct Sample {
    x: f64,
    y: f64,
    pressure: f64,
}

impl Sample {
    fn from_view(x: f64, y: f64, width: f64, height: f64, pressure: f64) -> Self {
        Self {
            x: (x / width.max(1.0)).clamp(0.0, 1.0),
            y: (y / height.max(1.0)).clamp(0.0, 1.0),
            pressure: pressure.clamp(0.0, 1.0),
        }
    }

    fn view_position(self, width: f64, height: f64) -> (f64, f64) {
        (self.x * width, self.y * height)
    }
}

#[derive(Default)]
struct Canvas {
    strokes: Vec<Vec<Sample>>,
    drawing: bool,
    samples: usize,
}

fn sheet_ratio(rotation: u32) -> f32 {
    if matches!(rotation, 0 | 2) {
        1.4
    } else {
        1.0 / 1.4
    }
}

pub fn pen_test() -> gtk4::Box {
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    root.set_vexpand(true);
    root.append(
        &gtk4::Label::builder()
            .label(
                "Draw with your pen. Stroke width follows the pressure received from your tablet.",
            )
            .wrap(true)
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build(),
    );
    let controls = adw::PreferencesGroup::new();
    let rotation_row = adw::ComboRow::builder()
        .title("Canvas rotation")
        .subtitle("Preview only · clears drawing · tablet settings stay unchanged")
        .model(&gtk4::StringList::new(&[
            "0° · Landscape",
            "90° · Portrait",
            "180° · Landscape",
            "270° · Portrait",
        ]))
        .build();
    controls.add(&rotation_row);
    root.append(&controls);

    let area = gtk4::DrawingArea::builder()
        .content_width(1)
        .content_height(1)
        .hexpand(true)
        .vexpand(true)
        .css_classes(["card"])
        .overflow(gtk4::Overflow::Hidden)
        .build();
    area.set_tooltip_text(Some(
        "Use a tablet stylus. Canvas rotation changes this sheet, while ink stays under your pen.",
    ));
    // The sheet rotates inside this workspace. Its aspect ratio constrains the
    // actual input widget, so no input-coordinate rotation or stretching is needed.
    let frame = gtk4::AspectFrame::builder()
        .ratio(sheet_ratio(0))
        .obey_child(false)
        .height_request(520)
        .hexpand(true)
        .vexpand(true)
        .child(&area)
        .build();
    root.append(&frame);
    let meter = gtk4::ProgressBar::new();
    root.append(&meter);
    let tools = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let label = gtk4::Label::builder()
        .label("Waiting for stylus pressure")
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .css_classes(["dim-label", "caption"])
        .build();
    let clear = gtk4::Button::builder()
        .label("Clear drawing")
        .css_classes(["flat"])
        .build();
    tools.append(&label);
    tools.append(&clear);
    root.append(&tools);

    let state = Rc::new(RefCell::new(Canvas::default()));
    let rotation = Rc::new(Cell::new(0));
    let draw_state = state.clone();
    let draw_rotation = rotation.clone();
    area.set_draw_func(move |area, cr, width, height| {
        let color = area.style_context().color();
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.04,
        );
        let _ = cr.paint();
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.18,
        );
        let mut x = 20;
        while x < width {
            let mut y = 20;
            while y < height {
                cr.arc(x as f64, y as f64, 0.7, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
                y += 20;
            }
            x += 20;
        }
        // The orientation marker makes 180°/270° observable as well as portrait.
        let (angle, gx, gy) = match draw_rotation.get() {
            1 => (
                std::f64::consts::FRAC_PI_2,
                width as f64 - 22.0,
                height as f64 / 2.0,
            ),
            2 => (
                std::f64::consts::PI,
                width as f64 / 2.0,
                height as f64 - 22.0,
            ),
            3 => (-std::f64::consts::FRAC_PI_2, 22.0, height as f64 / 2.0),
            _ => (0.0, width as f64 / 2.0, 22.0),
        };
        let _ = cr.save();
        cr.translate(gx, gy);
        cr.rotate(angle);
        cr.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            0.5,
        );
        cr.set_line_width(1.5);
        cr.move_to(-22.0, 5.0);
        cr.line_to(-22.0, -5.0);
        cr.move_to(-26.0, -1.0);
        cr.line_to(-22.0, -5.0);
        cr.line_to(-18.0, -1.0);
        let _ = cr.stroke();
        cr.set_font_size(11.0);
        cr.move_to(-12.0, 4.0);
        let _ = cr.show_text("TOP");
        let _ = cr.restore();
        cr.set_source_rgb(0.24, 0.52, 0.92);
        cr.set_line_cap(gtk4::cairo::LineCap::Round);
        let (w, h) = (width as f64, height as f64);
        for stroke in &draw_state.borrow().strokes {
            if stroke.len() == 1 {
                let p = stroke[0];
                let (x, y) = p.view_position(w, h);
                cr.arc(x, y, 0.5 + 7.0 * p.pressure, 0.0, std::f64::consts::TAU);
                let _ = cr.fill();
            }
            for pair in stroke.windows(2) {
                cr.set_line_width(1.0 + 14.0 * (pair[0].pressure + pair[1].pressure) / 2.0);
                let (x0, y0) = pair[0].view_position(w, h);
                let (x1, y1) = pair[1].view_position(w, h);
                cr.move_to(x0, y0);
                cr.line_to(x1, y1);
                let _ = cr.stroke();
            }
        }
    });
    let stylus = gtk4::GestureStylus::new();
    let down_state = state.clone();
    let down_area = area.downgrade();
    let down_label = label.clone();
    let down_meter = meter.clone();
    stylus.connect_down(move |gesture, x, y| {
        let Some(area) = down_area.upgrade() else {
            return;
        };
        let mut state = down_state.borrow_mut();
        if state.samples >= 50_000 {
            state.strokes.clear();
            state.samples = 0;
        }
        state.drawing = true;
        state.strokes.push(Vec::new());
        record_sample(gesture, &mut state, (x, y), &area, &down_label, &down_meter);
        area.queue_draw();
    });
    let motion_state = state.clone();
    let motion_area = area.downgrade();
    let motion_label = label.clone();
    let motion_meter = meter.clone();
    stylus.connect_motion(move |gesture, x, y| {
        let Some(area) = motion_area.upgrade() else {
            return;
        };
        let mut state = motion_state.borrow_mut();
        if state.drawing && state.samples < 50_000 {
            record_sample(
                gesture,
                &mut state,
                (x, y),
                &area,
                &motion_label,
                &motion_meter,
            );
            area.queue_draw();
        }
    });
    let up_state = state.clone();
    let up_label = label.clone();
    let up_meter = meter.clone();
    stylus.connect_up(move |_, _, _| {
        up_state.borrow_mut().drawing = false;
        up_meter.set_fraction(0.0);
        up_label.set_label("Pen lifted · pressure 0%");
    });
    let cancel_state = state.clone();
    let cancel_label = label.clone();
    let cancel_meter = meter.clone();
    stylus.connect_cancel(move |_, _| {
        cancel_state.borrow_mut().drawing = false;
        cancel_meter.set_fraction(0.0);
        cancel_label.set_label("Stroke interrupted · pressure 0%");
    });
    area.add_controller(stylus);
    let clear_state = state.clone();
    let clear_area = area.clone();
    let clear_label = label.clone();
    let clear_meter = meter.clone();
    clear.connect_clicked(move |_| {
        *clear_state.borrow_mut() = Canvas::default();
        clear_area.queue_draw();
        clear_meter.set_fraction(0.0);
        clear_label.set_label("Waiting for stylus pressure");
    });
    let rotation_area = area.downgrade();
    rotation_row.connect_selected_notify(move |row| {
        rotation.set(row.selected());
        frame.set_ratio(sheet_ratio(row.selected()));
        *state.borrow_mut() = Canvas::default();
        meter.set_fraction(0.0);
        label.set_label("Canvas rotated · drawing cleared");
        if let Some(area) = rotation_area.upgrade() {
            area.queue_draw();
        }
    });
    root
}

fn record_sample(
    gesture: &gtk4::GestureStylus,
    state: &mut Canvas,
    (x, y): (f64, f64),
    area: &gtk4::DrawingArea,
    label: &gtk4::Label,
    meter: &gtk4::ProgressBar,
) {
    if let Some(pressure) = gesture
        .axis(gtk4::gdk::AxisUse::Pressure)
        .filter(|p| p.is_finite())
    {
        let pressure = pressure.clamp(0.0, 1.0);
        meter.set_fraction(pressure);
        label.set_label(&format!("Stylus pressure · {:.0}%", pressure * 100.0));
        if let Some(stroke) = state.strokes.last_mut() {
            stroke.push(Sample::from_view(
                x,
                y,
                area.width() as f64,
                area.height() as f64,
                pressure,
            ));
            state.samples += 1;
        }
    } else {
        meter.set_fraction(0.0);
        label.set_label("No pressure axis received from this input device");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotating_sheet_alternates_landscape_and_portrait() {
        assert!(sheet_ratio(0) > 1.0);
        assert!(sheet_ratio(1) < 1.0);
        assert_eq!(sheet_ratio(0), sheet_ratio(2));
        assert_eq!(sheet_ratio(1), sheet_ratio(3));
        assert!((sheet_ratio(0) * sheet_ratio(1) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn corners_and_pressure_follow_cursor_for_every_sheet_orientation() {
        for rotation in 0..4 {
            let height = 520.0;
            let width = height * sheet_ratio(rotation) as f64;
            for (x, y) in [
                (0.0, 0.0),
                (width, 0.0),
                (0.0, height),
                (width, height),
                (width * 0.31, height * 0.72),
            ] {
                let sample = Sample::from_view(x, y, width, height, 0.37);
                let (sx, sy) = sample.view_position(width, height);
                assert!((sx - x).abs() < 1e-9);
                assert!((sy - y).abs() < 1e-9);
                assert_eq!(sample.pressure, 0.37);
                let (sx, sy) = sample.view_position(width * 0.5, height * 0.5);
                assert!((sx - x * 0.5).abs() < 1e-9);
                assert!((sy - y * 0.5).abs() < 1e-9);
            }
        }
    }
}
