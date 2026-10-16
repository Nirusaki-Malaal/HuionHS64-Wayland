//! HS64 cable-up reference: two keys at each upper corner, per Huion's manual.
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub const BUTTON_POSITIONS: [&str; 4] = ["Upper left", "Lower left", "Upper right", "Lower right"];

fn rounded_rectangle(
    cr: &gtk4::cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let half_pi = std::f64::consts::FRAC_PI_2;
    cr.new_sub_path();
    cr.arc(x + width - radius, y + radius, radius, -half_pi, 0.0);
    cr.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        half_pi,
    );
    cr.arc(
        x + radius,
        y + height - radius,
        radius,
        half_pi,
        2.0 * half_pi,
    );
    cr.arc(x + radius, y + radius, radius, 2.0 * half_pi, 3.0 * half_pi);
    cr.close_path();
}

pub fn diagram(actions: Rc<RefCell<[String; 4]>>) -> gtk4::DrawingArea {
    let area = gtk4::DrawingArea::builder()
        .content_height(310)
        .hexpand(true)
        .css_classes(["card"])
        .build();
    area.set_tooltip_text(Some("Cable at top: button 1 upper left, 2 below it, button 3 upper right, 4 below it. This reference stays fixed when you rotate the tablet."));
    area.set_draw_func(move |area, cr, width, height| {
        let scale = ((width as f64 - 24.0) / 560.0)
            .min((height as f64 - 20.0) / 430.0)
            .max(0.1);
        cr.translate(
            (width as f64 - 560.0 * scale) / 2.0,
            (height as f64 - 430.0 * scale) / 2.0,
        );
        cr.scale(scale, scale);
        let color = area.style_context().color();
        let foreground = |alpha: f64| {
            cr.set_source_rgba(
                color.red() as f64,
                color.green() as f64,
                color.blue() as f64,
                alpha,
            )
        };
        foreground(0.08);
        rounded_rectangle(cr, 60.0, 50.0, 440.0, 350.0, 20.0);
        let _ = cr.fill_preserve();
        foreground(0.5);
        cr.set_line_width(2.0);
        let _ = cr.stroke();
        // The USB port is near the left end of the top edge, as on the device.
        cr.move_to(132.0, 50.0);
        cr.line_to(132.0, 14.0);
        let _ = cr.stroke();
        cr.set_font_size(14.0);
        cr.move_to(144.0, 25.0);
        let _ = cr.show_text("USB cable");
        foreground(0.4);
        cr.set_font_size(17.0);
        cr.move_to(250.0, 100.0);
        let _ = cr.show_text("HUION");
        cr.set_line_width(1.0);
        cr.set_dash(&[5.0, 5.0], 0.0);
        rounded_rectangle(cr, 102.0, 178.0, 356.0, 180.0, 5.0);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
        cr.set_font_size(17.0);
        cr.move_to(225.0, 273.0);
        let _ = cr.show_text("Drawing area");
        let actions = actions.borrow();
        for (index, &(x, y)) in [(92.0, 80.0), (92.0, 120.0), (436.0, 80.0), (436.0, 120.0)]
            .iter()
            .enumerate()
        {
            cr.set_source_rgb(0.24, 0.52, 0.92);
            rounded_rectangle(cr, x, y, 32.0, 28.0, 6.0);
            let _ = cr.fill();
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_font_size(18.0);
            cr.move_to(x + 10.0, y + 20.0);
            let _ = cr.show_text(&(index + 1).to_string());
            foreground(0.85);
            cr.set_font_size(15.0);
            let text = &actions[index];
            let text_width = cr
                .text_extents(text)
                .map(|extents| extents.width())
                .unwrap_or(0.0);
            let label_x = if index < 2 {
                x + 44.0
            } else {
                x - text_width - 12.0
            };
            cr.move_to(label_x, y + 20.0);
            let _ = cr.show_text(text);
        }
    });
    area
}
