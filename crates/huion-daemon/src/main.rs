mod dbus;
mod display;
mod hidraw;
mod uinput;

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{mpsc, Mutex as TokioMutex, RwLock};
use tracing::{error, info, warn};

use huion_proto::config::TabletConfig;
use huion_proto::packet::{parse_report, TabletReport};

use crate::dbus::HuionDbusService;
use crate::hidraw::{find_huion_devices, DeviceInfo};
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
    let primary_device_info = Arc::new(RwLock::new(None::<DeviceInfo>));

    // Initialize D-Bus session connection
    let dbus_service = HuionDbusService::new(config.clone(), primary_device_info.clone());
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
    let virtual_tablet = match VirtualTablet::new() {
        Ok(vt) => {
            info!("Created virtual uinput tablet device: 'Huion HS64 Wayland Stylus/Pad'");
            Some(vt)
        }
        Err(e) => {
            error!(
                "Failed to create /dev/uinput virtual device: {}. Check user access to /dev/uinput or udev rules.",
                e
            );
            None
        }
    };

    let vt_holder = Arc::new(TokioMutex::new(virtual_tablet));
    let (packet_tx, mut packet_rx) = mpsc::channel::<(PathBuf, Vec<u8>)>(256);
    let active_paths = Arc::new(StdMutex::new(HashSet::<PathBuf>::new()));

    // Task 1: Hardware reader dispatcher and monitor
    let active_paths_monitor = active_paths.clone();
    let packet_tx_monitor = packet_tx.clone();
    let primary_info_holder = primary_device_info.clone();

    tokio::spawn(async move {
        loop {
            let devices = find_huion_devices();
            {
                let mut guard = primary_info_holder.write().await;
                *guard = devices.first().cloned();
            }

            for dev in devices {
                let mut paths = active_paths_monitor.lock().unwrap();
                if !paths.contains(&dev.path) {
                    paths.insert(dev.path.clone());
                    let path_to_read = dev.path.clone();
                    let tx = packet_tx_monitor.clone();
                    let paths_cleanup = active_paths_monitor.clone();

                    info!("Attaching input stream reader to {:?}", path_to_read);
                    tokio::task::spawn_blocking(move || {
                        let mut file = match File::open(&path_to_read) {
                            Ok(f) => f,
                            Err(e) => {
                                warn!("Could not open {:?}: {}", path_to_read, e);
                                if let Ok(mut set) = paths_cleanup.lock() {
                                    set.remove(&path_to_read);
                                }
                                return;
                            }
                        };

                        let mut buf = [0u8; 64];
                        loop {
                            match file.read(&mut buf) {
                                Ok(0) => {
                                    info!("Interface {:?} closed (EOF).", path_to_read);
                                    break;
                                }
                                Ok(n) => {
                                    if tx.blocking_send((path_to_read.clone(), buf[..n].to_vec())).is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Read error on {:?}: {}", path_to_read, e);
                                    break;
                                }
                            }
                        }

                        if let Ok(mut set) = paths_cleanup.lock() {
                            set.remove(&path_to_read);
                        }
                    });
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    // Task 2: Central Packet Processor & Virtual Input Emitter
    let config_processor = config.clone();
    let vt_processor = vt_holder.clone();

    tokio::spawn(async move {
        while let Some((source_path, data)) = packet_rx.recv().await {
            match parse_report(&data) {
                Ok(Some(report)) => {
                    let mut vt_guard = vt_processor.lock().await;
                    if let Some(ref mut vt) = *vt_guard {
                        let current_cfg = config_processor.read().await;
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
                    tracing::trace!("Packet parse note from {:?}: {}", source_path, e);
                }
            }
        }
    });

    info!("Daemon running. Awaiting hardware events or termination (Ctrl+C)...");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down Huion Wayland Daemon.");

    Ok(())
}
