use std::sync::Arc;
use tokio::sync::RwLock;
use zbus::interface;

use crate::display::detect_displays;
use crate::hidraw::DeviceInfo;
use huion_proto::config::TabletConfig;

pub struct HuionDbusService {
    config: Arc<RwLock<TabletConfig>>,
    device_info: Arc<RwLock<Option<DeviceInfo>>>,
}

impl HuionDbusService {
    pub fn new(
        config: Arc<RwLock<TabletConfig>>,
        device_info: Arc<RwLock<Option<DeviceInfo>>>,
    ) -> Self {
        Self {
            config,
            device_info,
        }
    }
}

#[interface(name = "org.wayland.HuionTablet")]
impl HuionDbusService {
    async fn get_config(&self) -> String {
        let cfg = self.config.read().await;
        serde_json::to_string(&*cfg).unwrap_or_default()
    }

    async fn set_config(&self, json_str: String) -> bool {
        match serde_json::from_str::<TabletConfig>(&json_str) {
            Ok(new_cfg) => {
                if let Err(e) = crate::uinput::validate_config(&new_cfg) {
                    tracing::warn!("Rejected invalid configuration: {}", e);
                    return false;
                }
                // Serialize disk and memory updates so concurrent clients cannot
                // leave one configuration saved and another running.
                let mut cfg = self.config.write().await;
                if let Err(e) = new_cfg.save() {
                    tracing::error!("Failed to persist config to disk: {}", e);
                    return false;
                }
                *cfg = new_cfg;
                tracing::info!("Applied updated configuration via D-Bus");
                true
            }
            Err(e) => {
                tracing::error!("Failed to parse incoming config JSON: {}", e);
                false
            }
        }
    }

    async fn get_device_info(&self) -> String {
        let dev = self.device_info.read().await;
        match &*dev {
            Some(d) => serde_json::json!({
                "connected": true,
                "name": d.name,
                "vendor_id": d.vendor_id,
                "product_id": d.product_id,
                "path": d.path.to_string_lossy(),
            })
            .to_string(),
            None => serde_json::json!({ "connected": false }).to_string(),
        }
    }

    async fn get_displays(&self) -> String {
        let displays = detect_displays();
        serde_json::to_string(&displays).unwrap_or_default()
    }
}
