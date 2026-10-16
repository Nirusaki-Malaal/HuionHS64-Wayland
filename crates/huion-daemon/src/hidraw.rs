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

                if vid == HUION_VENDOR_ID && pid == 0x006f {
                    let dev_name = entry.file_name();
                    let dev_path = Path::new("/dev").join(dev_name);
                    tracing::debug!(
                        "Discovered Huion interface at {:?}: VID=0x{:04x}, PID=0x{:04x}, Name={}",
                        dev_path,
                        vid,
                        pid,
                        name
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
    if let Some(first) = devices.first() {
        let first_name = first.path.file_name().unwrap();
        let first_sys = fs::canonicalize(
            Path::new("/sys/class/hidraw")
                .join(first_name)
                .join("device"),
        )
        .ok();
        let usb_parent = first_sys
            .as_ref()
            .and_then(|p| p.ancestors().find(|p| p.join("idVendor").exists()))
            .map(Path::to_path_buf);
        if let Some(parent) = usb_parent {
            devices.retain(|d| {
                fs::canonicalize(
                    Path::new("/sys/class/hidraw")
                        .join(d.path.file_name().unwrap())
                        .join("device"),
                )
                .is_ok_and(|p| p.starts_with(&parent))
            });
        }
    }
    devices
}

/// Convenience function returning the primary device interface
#[allow(dead_code)]
pub fn find_huion_device() -> Option<DeviceInfo> {
    find_huion_devices().into_iter().next()
}

/// Grab only the physical USB input nodes belonging to the chosen tablet.
/// Keeping these handles alive suppresses kernel fallback events; dropping them
/// restores normal kernel delivery even after a crash or unplug.
pub fn physical_input_paths(devices: &[DeviceInfo]) -> std::io::Result<Vec<PathBuf>> {
    use std::io;
    let mut usb_parents = Vec::new();
    for device in devices {
        let raw_name = device
            .path
            .file_name()
            .ok_or_else(|| io::Error::other("Invalid hidraw path"))?;
        let raw_sysfs =
            fs::canonicalize(Path::new("/sys/class/hidraw").join(raw_name).join("device"))?;
        let parent = raw_sysfs
            .ancestors()
            .find(|p| p.join("idVendor").exists())
            .ok_or_else(|| io::Error::other("Missing USB parent"))?
            .to_path_buf();
        if !usb_parents.contains(&parent) {
            usb_parents.push(parent);
        }
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir("/sys/class/input")? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("event") {
            continue;
        }
        let target = fs::canonicalize(entry.path())?;
        if !usb_parents.iter().any(|p| target.starts_with(p)) {
            continue;
        }
        paths.push(Path::new("/dev/input").join(entry.file_name()));
    }
    paths.sort();
    Ok(paths)
}

pub fn grab_physical_devices(paths: &[PathBuf]) -> std::io::Result<Vec<evdev::Device>> {
    use std::io;
    let mut grabbed = Vec::new();
    for path in paths {
        let mut input = evdev::Device::open(path)?;
        input.grab().map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Cannot exclusively grab {}: {e}", path.display()),
            )
        })?;
        tracing::info!("Exclusively grabbed physical input {}", path.display());
        grabbed.push(input);
    }
    // hidraw and evdev nodes appear asynchronously on hotplug. Require both
    // observed HS64 functions before replacing native input, not just one node.
    let has_pen = grabbed.iter().any(|device| {
        device
            .supported_absolute_axes()
            .is_some_and(|axes| axes.contains(evdev::AbsoluteAxisType::ABS_PRESSURE))
    });
    let has_keyboard = grabbed.iter().any(|device| {
        device
            .supported_keys()
            .is_some_and(|keys| keys.contains(evdev::Key::KEY_Z))
    });
    if !has_pen || !has_keyboard {
        return Err(io::Error::other(
            "Waiting for both HS64 pen and keyboard evdev interfaces",
        ));
    }
    Ok(grabbed)
}
