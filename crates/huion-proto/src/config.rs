use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", content = "value")]
pub enum KeyAction {
    #[default]
    Disabled,
    Key(String),           // e.g. "KEY_Z"
    KeyCombo(Vec<String>), // e.g. ["KEY_LEFTCTRL", "KEY_Z"]
    MouseButton(u16),      // e.g. BTN_LEFT, BTN_MIDDLE, BTN_RIGHT
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
        self.save_to(&Self::config_path())
    }

    fn save_to(&self, path: &Path) -> std::io::Result<()> {
        self.validate()
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        atomic_write(path, serialized.as_bytes())
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

// Keep the temporary file beside its destination so rename is atomic. A failed
// write/sync/rename leaves the existing configuration untouched. create_new
// prevents following a preexisting symlink or overwriting another writer's file.
fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let (temp_path, mut file) = loop {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".huion-config-{}-{counter}.tmp",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => break (temp_path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp_path, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    result
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            loop {
                let path = std::env::temp_dir().join(format!(
                    "huion-config-test-{}-{}",
                    std::process::id(),
                    COUNTER.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("cannot create test directory: {error}"),
                }
            }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn saving_replaces_complete_configuration_and_invalid_save_preserves_it() {
        let directory = TempDir::new();
        let path = directory.0.join("config.toml");
        std::fs::write(&path, "old configuration").unwrap();
        let mut config = TabletConfig::default();
        config.save_to(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(toml::from_str::<TabletConfig>(&saved).unwrap(), config);
        config.active_area.width = 0;
        assert!(config.save_to(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_rename_preserves_destination_and_cleans_temporary_file() {
        let directory = TempDir::new();
        let destination = directory.0.join("config.toml");
        std::fs::create_dir(&destination).unwrap();
        let existing = destination.join("keep");
        std::fs::write(&existing, "original").unwrap();
        assert!(TabletConfig::default().save_to(&destination).is_err());
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "original");
        assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}
