use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayOutput {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub connected: bool,
}

pub fn detect_displays() -> Vec<DisplayOutput> {
    let mut outputs = Vec::new();
    let drm_path = Path::new("/sys/class/drm");
    if !drm_path.exists() {
        return vec![DisplayOutput {
            name: "Default".to_string(),
            width: 1920,
            height: 1080,
            connected: true,
        }];
    }

    if let Ok(entries) = fs::read_dir(drm_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let status_path = path.join("status");
            if !status_path.exists() {
                continue;
            }

            if let Ok(status) = fs::read_to_string(&status_path) {
                if status.trim() == "connected" {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    let name = if let Some(s) = fname.strip_prefix("card0-") {
                        s.to_string()
                    } else if let Some(s) = fname.strip_prefix("card1-") {
                        s.to_string()
                    } else {
                        fname
                    };

                    // Read current mode
                    let modes_path = path.join("modes");
                    let mut width = 1920;
                    let mut height = 1080;
                    if let Ok(modes) = fs::read_to_string(&modes_path) {
                        if let Some(first_mode) = modes.lines().next() {
                            let parts: Vec<&str> = first_mode.split('x').collect();
                            if parts.len() == 2 {
                                width = parts[0].parse().unwrap_or(1920);
                                height = parts[1].parse().unwrap_or(1080);
                            }
                        }
                    }

                    debug!("Detected active display {}: {}x{}", name, width, height);
                    outputs.push(DisplayOutput {
                        name,
                        width,
                        height,
                        connected: true,
                    });
                }
            }
        }
    }

    if outputs.is_empty() {
        outputs.push(DisplayOutput {
            name: "Primary".to_string(),
            width: 1920,
            height: 1080,
            connected: true,
        });
    }

    outputs
}
