mod dbus;
mod display;
mod hidraw;
mod uinput;

use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use huion_proto::config::TabletConfig;
use huion_proto::packet::{parse_report, TabletReport};

use crate::dbus::HuionDbusService;
use crate::hidraw::{find_huion_device, DeviceInfo};
use crate::uinput::VirtualTablet;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Huion HS64 Native Wayland Driver Daemon v{}", env!("CARGO_PKG_VERSION"));

    let config = Arc::new(RwLock::new(TabletConfig::load_or_default()));
    let device_info = Arc::new(RwLock::new(None::<DeviceInfo>));

    // Initialize D-Bus session connection
    let dbus_service = HuionDbusService::new(config.clone(), device_info.clone());
    let _conn = match zbus::connection::Builder::session() {
        Ok(builder) => match builder.name("org.wayland.HuionTablet") {
            Ok(builder) => match builder.serve_at("/org/wayland/HuionTablet", dbus_service) {
                Ok(builder) => match builder.build().await {
                    Ok(conn) => {
                        info!("Registered D-Bus service at org.wayland.HuionTablet");
                        Some(conn)
                    }
                    Err(e) => {
                        warn!("Could not connect to D-Bus session bus: {}. Running headless.", e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Could not attach D-Bus interface: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Could not request D-Bus name: {}", e);
                None
            }
        },
        Err(e) => {
            warn!("Could not build D-Bus connection: {}", e);
            None
        }
    };

    // Initialize Virtual Tablet Uinput Device
    let mut virtual_tablet = match VirtualTablet::new() {
        Ok(vt) => {
            info!("Created virtual uinput tablet device: 'Huion HS64 Wayland Stylus/Pad'");
            Some(vt)
        }
        Err(e) => {
            error!(
                "Failed to create /dev/uinput virtual device: {}. Check that user has rw access to /dev/uinput or udev rule is installed.",
                e
            );
            None
        }
    };

    let config_clone = config.clone();
    let device_info_clone = device_info.clone();

    // Spawn device poller and event processor
    tokio::spawn(async move {
        let mut buffer = [0u8; 64];

        loop {
            let dev = find_huion_device();
            {
                let mut info_guard = device_info_clone.write().await;
                *info_guard = dev.clone();
            }

            if let Some(dev) = dev {
                info!("Opening tablet stream at {:?}", dev.path);
                match File::open(&dev.path) {
                    Ok(mut file) => loop {
                        match file.read(&mut buffer) {
                            Ok(0) => {
                                warn!("EOF on tablet stream. Device likely disconnected.");
                                break;
                            }
                            Ok(bytes_read) => {
                                let slice = &buffer[..bytes_read];
                                match parse_report(slice) {
                                    Ok(Some(report)) => {
                                        if let Some(ref mut vt) = virtual_tablet {
                                            let current_cfg = config_clone.read().await;
                                            match report {
                                                TabletReport::Stylus(s) => {
                                                    let _ = vt.emit_stylus(s, &current_cfg);
                                                }
                                                TabletReport::ExpressKeys(k) => {
                                                    let _ = vt.emit_express_keys(k);
                                                }
                                                TabletReport::Raw(_) => {}
                                            }
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        tracing::trace!("Packet parse note: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Error reading from tablet: {}. Reconnecting...", e);
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        warn!("Could not open {:?}: {}. Retrying in 2s...", dev.path, e);
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    info!("Daemon running. Awaiting hardware events or termination (Ctrl+C)...");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down Huion Wayland Daemon.");

    Ok(())
}
