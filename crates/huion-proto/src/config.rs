use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::bezier::PressureCurve;
use crate::packet::{HS64_MAX_X, HS64_MAX_Y};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    #[default]
    Normal, // 0 degrees
    Rotate90,  // 90 degrees clockwise (Portrait)
    Rotate180, // 180 degrees (Left-Handed mode)
    Rotate270, // 270 degrees clockwise
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ActiveArea {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Default for ActiveArea {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: HS64_MAX_X,
            height: HS64_MAX_Y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum KeyAction {
    Disabled,
    Key(String),           // e.g. "KEY_Z"
    KeyCombo(Vec<String>), // e.g. ["KEY_LEFTCTRL", "KEY_Z"]
    MouseButton(u16),      // e.g. BTN_LEFT, BTN_MIDDLE, BTN_RIGHT
}

impl Default for KeyAction {
    fn default() -> Self {
        KeyAction::Disabled
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabletConfig {
    pub device_name: String,
    pub orientation: Orientation,
    pub active_area: ActiveArea,
    pub aspect_ratio_locked: bool,
    pub pressure_curve: PressureCurve,
    pub express_keys: [KeyAction; 4],
    pub target_output: Option<String>,
}

impl Default for TabletConfig {
    fn default() -> Self {
        Self {
            device_name: "Huion HS64".to_string(),
            orientation: Orientation::Normal,
            active_area: ActiveArea::default(),
            aspect_ratio_locked: false,
            pressure_curve: PressureCurve::default(),
            express_keys: [
                KeyAction::KeyCombo(vec!["KEY_LEFTCTRL".to_string(), "KEY_Z".to_string()]), // Undo
                KeyAction::KeyCombo(vec![
                    "KEY_LEFTCTRL".to_string(),
                    "KEY_LEFTSHIFT".to_string(),
                    "KEY_Z".to_string(),
                ]), // Redo
                KeyAction::Key("KEY_B".to_string()), // Brush tool
                KeyAction::Key("KEY_E".to_string()), // Eraser tool
            ],
            target_output: None, // All outputs by default
        }
    }
}

impl TabletConfig {
    /// Validate values before persisting or applying configuration. The daemon
    /// additionally checks key names against its actual evdev capabilities.
    pub fn validate(&self) -> Result<(), String> {
        if self.device_name.trim().is_empty() {
            return Err("device_name must not be empty".into());
        }
        let area = self.active_area;
        if area.width == 0 || area.height == 0 {
            return Err("active area width and height must be positive".into());
        }
        if u32::from(area.x) + u32::from(area.width) > u32::from(HS64_MAX_X)
            || u32::from(area.y) + u32::from(area.height) > u32::from(HS64_MAX_Y)
        {
            return Err("active area must fit within the tablet bounds".into());
        }
        for point in [self.pressure_curve.p1, self.pressure_curve.p2] {
            if !point.x.is_finite()
                || !point.y.is_finite()
                || !(0.0..=1.0).contains(&point.x)
                || !(0.0..=1.0).contains(&point.y)
            {
                return Err("pressure curve points must be finite values between 0 and 1".into());
            }
        }
        if self
            .target_output
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err("target_output must not be empty".into());
        }
        fn valid_key(name: &str) -> bool {
            name.strip_prefix("KEY_").is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix
                        .bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            })
        }
        for (index, action) in self.express_keys.iter().enumerate() {
            let valid = match action {
                KeyAction::Disabled => true,
                KeyAction::Key(name) => valid_key(name),
                KeyAction::KeyCombo(keys) => {
                    !keys.is_empty() && keys.iter().all(|name| valid_key(name))
                }
                KeyAction::MouseButton(code) => (272..=279).contains(code),
            };
            if !valid {
                return Err(format!("invalid action for express key {}", index + 1));
            }
        }
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("HOME").unwrap_or_default();
                PathBuf::from(home).join(".config")
            });
        base.join("huion-wayland").join("config.toml")
    }

    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<TabletConfig>(&content) {
                    if cfg.validate().is_ok() {
                        return cfg;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.validate()
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, serialized)
    }

    pub fn transform_coordinates(&self, raw_x: u16, raw_y: u16) -> (u16, u16) {
        // Clamp to active area
        let clamped_x = raw_x.clamp(
            self.active_area.x,
            self.active_area.x.saturating_add(self.active_area.width),
        );
        let clamped_y = raw_y.clamp(
            self.active_area.y,
            self.active_area.y.saturating_add(self.active_area.height),
        );

        // Normalize to 0..HS64_MAX_X, 0..HS64_MAX_Y
        let rel_x = clamped_x.saturating_sub(self.active_area.x);
        let rel_y = clamped_y.saturating_sub(self.active_area.y);

        let norm_x = if self.active_area.width > 0 {
            ((rel_x as u32 * HS64_MAX_X as u32) / self.active_area.width as u32) as u16
        } else {
            0
        };

        let norm_y = if self.active_area.height > 0 {
            ((rel_y as u32 * HS64_MAX_Y as u32) / self.active_area.height as u32) as u16
        } else {
            0
        };

        // Apply orientation rotation
        match self.orientation {
            Orientation::Normal => (norm_x, norm_y),
            Orientation::Rotate180 => (
                HS64_MAX_X.saturating_sub(norm_x),
                HS64_MAX_Y.saturating_sub(norm_y),
            ),
            // Rotate in normalized space, then restore each output axis's
            // own range. Swapping raw X/Y overflowed the shorter Y axis.
            Orientation::Rotate90 => (
                HS64_MAX_X
                    - ((u32::from(norm_y) * u32::from(HS64_MAX_X)) / u32::from(HS64_MAX_Y)) as u16,
                ((u32::from(norm_x) * u32::from(HS64_MAX_Y)) / u32::from(HS64_MAX_X)) as u16,
            ),
            Orientation::Rotate270 => (
                ((u32::from(norm_y) * u32::from(HS64_MAX_X)) / u32::from(HS64_MAX_Y)) as u16,
                HS64_MAX_Y
                    - ((u32::from(norm_x) * u32::from(HS64_MAX_Y)) / u32::from(HS64_MAX_X)) as u16,
            ),
        }
    }
}
