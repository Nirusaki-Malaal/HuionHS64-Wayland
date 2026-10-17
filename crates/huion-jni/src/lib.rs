use jni::objects::{JByteArray, JClass, JFloatArray, JIntArray};
use jni::sys::{jboolean, jfloat, jint};
use jni::JNIEnv;

use huion_proto::bezier::PressureCurve;
use huion_proto::config::{ActiveArea, Orientation, TabletConfig};
use huion_proto::packet::{parse_report, TabletReport};

#[no_mangle]
pub extern "system" fn Java_io_github_nirusaki_kalam_bridge_NativeBridge_applyPressureCurve(
    _env: JNIEnv,
    _class: JClass,
    raw_pressure: jint,
    p1x: jfloat,
    p1y: jfloat,
    p2x: jfloat,
    p2y: jfloat,
) -> jint {
    let curve = PressureCurve::new(p1x as f64, p1y as f64, p2x as f64, p2y as f64);
    let clamped = (raw_pressure.clamp(0, 8191) as f64) / 8191.0;
    let evaluated = curve.evaluate(clamped);
    (evaluated * 8191.0).round() as jint
}

#[no_mangle]
pub extern "system" fn Java_io_github_nirusaki_kalam_bridge_NativeBridge_transformCoordinates(
    env: JNIEnv,
    _class: JClass,
    x: jint,
    y: jint,
    orientation_deg: jint,
    out_coords: JIntArray,
) {
    let orientation = match orientation_deg {
        90 => Orientation::Rotate90,
        180 => Orientation::Rotate180,
        270 => Orientation::Rotate270,
        _ => Orientation::Normal,
    };
    let config = TabletConfig {
        orientation,
        active_area: ActiveArea::default(),
        ..Default::default()
    };
    let (tx, ty) = config.transform_coordinates(x.clamp(0, 40000) as u16, y.clamp(0, 25000) as u16);
    let coords: [jint; 2] = [tx as jint, ty as jint];
    let _ = env.set_int_array_region(&out_coords, 0, &coords);
}

#[no_mangle]
pub extern "system" fn Java_io_github_nirusaki_kalam_bridge_NativeBridge_parseUsbReport(
    env: JNIEnv,
    _class: JClass,
    raw_buffer: JByteArray,
    out_payload: JIntArray,
) -> jboolean {
    let Ok(bytes) = env.convert_byte_array(&raw_buffer) else {
        return 0;
    };

    match parse_report(&bytes) {
        Ok(Some(TabletReport::Stylus(s))) => {
            let data: [jint; 7] = [
                1, // Type: Stylus
                s.x as jint,
                s.y as jint,
                s.pressure as jint,
                s.touch as jint,
                s.barrel_switch as jint,
                s.in_range as jint,
            ];
            let _ = env.set_int_array_region(&out_payload, 0, &data);
            1
        }
        Ok(Some(TabletReport::ExpressKeys(k))) => {
            let data: [jint; 7] = [
                2, // Type: Keys
                k.key1 as jint,
                k.key2 as jint,
                k.key3 as jint,
                k.key4 as jint,
                0,
                0,
            ];
            let _ = env.set_int_array_region(&out_payload, 0, &data);
            1
        }
        _ => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_nirusaki_kalam_bridge_NativeBridge_sampleCurveLUT(
    env: JNIEnv,
    _class: JClass,
    p1x: jfloat,
    p1y: jfloat,
    p2x: jfloat,
    p2y: jfloat,
    out_samples: JFloatArray,
) {
    let curve = PressureCurve::new(p1x as f64, p1y as f64, p2x as f64, p2y as f64);
    let mut float_samples = [0.0f32; 256];
    for i in 0..256 {
        let input_ratio = i as f64 / 255.0;
        let evaluated = curve.evaluate(input_ratio);
        float_samples[i] = evaluated as f32;
    }
    let _ = env.set_float_array_region(&out_samples, 0, &float_samples);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_pressure_curve() {
        let curve = PressureCurve::default();
        let eval = (curve.evaluate(0.5) * 8191.0).round() as i32;
        assert!((eval - 4096).abs() <= 50);
    }
}
