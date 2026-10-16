use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BezierPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PressureCurve {
    pub p1: BezierPoint,
    pub p2: BezierPoint,
}

impl Default for PressureCurve {
    fn default() -> Self {
        // Linear 1:1 default response
        Self {
            p1: BezierPoint { x: 0.25, y: 0.25 },
            p2: BezierPoint { x: 0.75, y: 0.75 },
        }
    }
}

impl PressureCurve {
    pub fn new(p1x: f64, p1y: f64, p2x: f64, p2y: f64) -> Self {
        Self {
            p1: BezierPoint {
                x: p1x.clamp(0.0, 1.0),
                y: p1y.clamp(0.0, 1.0),
            },
            p2: BezierPoint {
                x: p2x.clamp(0.0, 1.0),
                y: p2y.clamp(0.0, 1.0),
            },
        }
    }

    /// Evaluates output pressure ratio [0.0, 1.0] given input pressure ratio [0.0, 1.0]
    pub fn evaluate(&self, input: f64) -> f64 {
        let x = input.clamp(0.0, 1.0);
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }

        // Binary search for parameter t where B_x(t) == x
        let mut low = 0.0;
        let mut high = 1.0;
        let mut t = x;

        for _ in 0..16 {
            let current_x = self.bezier_x(t);
            if (current_x - x).abs() < 1e-4 {
                break;
            }
            if current_x < x {
                low = t;
            } else {
                high = t;
            }
            t = (low + high) * 0.5;
        }

        self.bezier_y(t).clamp(0.0, 1.0)
    }

    /// Transforms raw integer pressure (0..8191) according to this curve
    pub fn map_pressure(&self, raw: u16, max_pressure: u16) -> u16 {
        if raw == 0 || max_pressure == 0 {
            return 0;
        }
        let input_ratio = (raw as f64) / (max_pressure as f64);
        let output_ratio = self.evaluate(input_ratio);
        (output_ratio * (max_pressure as f64)).round() as u16
    }

    #[inline]
    fn bezier_x(&self, t: f64) -> f64 {
        let u = 1.0 - t;
        // B(t) = 3*(1-t)^2*t*P1.x + 3*(1-t)*t^2*P2.x + t^3
        3.0 * u * u * t * self.p1.x + 3.0 * u * t * t * self.p2.x + t * t * t
    }

    #[inline]
    fn bezier_y(&self, t: f64) -> f64 {
        let u = 1.0 - t;
        // B(t) = 3*(1-t)^2*t*P1.y + 3*(1-t)*t^2*P2.y + t^3
        3.0 * u * u * t * self.p1.y + 3.0 * u * t * t * self.p2.y + t * t * t
    }
}
