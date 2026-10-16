use std::fs;
use std::path::{Path, PathBuf};

pub const HUION_VENDOR_ID: u16 = 0x256c;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub path: PathBuf,
    pub vendor_id: u16,
    pub product_id: u16,
    pub name: String,
}

/// Scans sysfs `/sys/class/hidraw` to locate all connected Huion tablet interfaces
pub fn find_huion_devices() -> Vec<DeviceInfo> {
    let hidraw_dir = Path::new("/sys/class/hidraw");
    let mut devices = Vec::new();
    if !hidraw_dir.exists() {
        return devices;
    }

    if let Ok(entries) = fs::read_dir(hidraw_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let uevent_path = path.join("device/uevent");
            if !uevent_path.exists() {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&uevent_path) {
                let mut vid = 0u16;
                let mut pid = 0u16;
                let mut name = String::new();

                for line in content.lines() {
                    if let Some(id_str) = line.strip_prefix("HID_ID=") {
                        let parts: Vec<&str> = id_str.split(':').collect();
                        if parts.len() >= 3 {
                            vid = u16::from_str_radix(parts[1], 16).unwrap_or(0);
                            pid = u16::from_str_radix(parts[2], 16).unwrap_or(0);
                        }
                    } else if let Some(n) = line.strip_prefix("HID_NAME=") {
                        name = n.to_string();
                    }
                }

                if vid == HUION_VENDOR_ID {
                    let dev_name = entry.file_name();
                    let dev_path = Path::new("/dev").join(dev_name);
                    tracing::debug!(
                        "Discovered Huion interface at {:?}: VID=0x{:04x}, PID=0x{:04x}, Name={}",
                        dev_path, vid, pid, name
                    );
                    devices.push(DeviceInfo {
                        path: dev_path,
                        vendor_id: vid,
                        product_id: pid,
                        name,
                    });
                }
            }
        }
    }

    devices.sort_by(|a, b| a.path.cmp(&b.path));
    devices
}

/// Convenience function returning the primary device interface
#[allow(dead_code)]
pub fn find_huion_device() -> Option<DeviceInfo> {
    find_huion_devices().into_iter().next()
}
