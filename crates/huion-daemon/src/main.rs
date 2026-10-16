mod dbus;
mod display;
mod hidraw;
mod uinput;

use crate::{
    dbus::HuionDbusService,
    hidraw::{find_huion_devices, DeviceInfo},
    uinput::{validate_config, VirtualTablet},
};
use huion_proto::{
    config::TabletConfig,
    packet::{parse_report, TabletReport},
};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::unix::AsyncFd,
    sync::{mpsc, RwLock},
    task::JoinHandle,
};
use tracing::{info, warn};

/// Runtime ownership: dropping a session cancels readers, releases virtual input
/// and ungrabs physical evdev nodes. No persistent libinput ignore rules needed.
struct Session {
    readers: Vec<JoinHandle<()>>,
    tablet: VirtualTablet,
    _grabs: Vec<evdev::Device>,
    paths: Vec<PathBuf>,
    generation: u64,
}
impl Drop for Session {
    fn drop(&mut self) {
        for reader in &self.readers {
            reader.abort();
        }
    }
}

enum HardwareEvent {
    Report(u64, Vec<u8>),
    Disconnected(u64),
}

fn singleton() -> anyhow::Result<File> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or_else(|| {
        anyhow::anyhow!("XDG_RUNTIME_DIR is required; run in your desktop user session")
    })?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(PathBuf::from(runtime).join("huion-wayland.lock"))?;
    file.try_lock()
        .map_err(|e| anyhow::anyhow!("Another Huion daemon owns this session: {e}"))?;
    Ok(file)
}

impl Session {
    fn start(
        devices: &[DeviceInfo],
        generation: u64,
        tx: mpsc::Sender<HardwareEvent>,
    ) -> anyhow::Result<Self> {
        let tablet = VirtualTablet::new()?;
        // Open every raw stream before suppressing native events, so a permission
        // failure leaves the physical tablet usable.
        let files: Vec<_> = devices
            .iter()
            .map(|d| {
                let file = OpenOptions::new()
                    .read(true)
                    .custom_flags(nix::libc::O_NONBLOCK)
                    .open(&d.path)?;
                // SAFETY: AsyncFd owns this File; no code closes or replaces its fd.
                unsafe { AsyncFd::register(file) }.map_err(io::Error::from)
            })
            .collect::<io::Result<_>>()?;
        let grabs = hidraw::grab_physical_devices(devices)?;
        let mut readers = Vec::new();
        for file in files {
            let sender = tx.clone();
            readers.push(tokio::spawn(async move {
                let mut buf = [0; 64];
                loop {
                    let mut ready = match file.readable().await {
                        Ok(r) => r,
                        Err(_) => break,
                    };
                    match ready.try_io(|fd| fd.get_ref().read(&mut buf)) {
                        Ok(Ok(0)) => break,
                        Ok(Ok(n)) => {
                            if sender
                                .send(HardwareEvent::Report(generation, buf[..n].to_vec()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Ok(Err(e)) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Ok(Err(e)) => {
                            warn!("Tablet read failed: {e}");
                            break;
                        }
                        Err(_) => continue,
                    }
                }
                let _ = sender.send(HardwareEvent::Disconnected(generation)).await;
            }));
        }
        Ok(Self {
            readers,
            tablet,
            _grabs: grabs,
            paths: devices.iter().map(|d| d.path.clone()).collect(),
            generation,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let _lock = singleton()?;
    let mut initial_config = TabletConfig::load_or_default();
    if let Err(e) = validate_config(&initial_config) {
        warn!("Saved configuration invalid: {e}; using defaults");
        initial_config = TabletConfig::default();
    }
    let config = Arc::new(RwLock::new(initial_config));
    let device_info = Arc::new(RwLock::new(None::<DeviceInfo>));
    // A name conflict is fatal: continuing headless would create a second cursor.
    let _connection = zbus::connection::Builder::session()?
        .name("org.wayland.HuionTablet")?
        .serve_at(
            "/org/wayland/HuionTablet",
            HuionDbusService::new(config.clone(), device_info.clone()),
        )?
        .build()
        .await?;
    let (tx, mut rx) = mpsc::channel(256);
    let mut session: Option<Session> = None;
    let mut generation = 0;
    let mut monitor = tokio::time::interval(Duration::from_secs(1));
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    info!("Huion daemon ready; waiting for supported HS64 hardware");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = terminate.recv() => break,
            _ = monitor.tick() => {
                let devices = find_huion_devices();
                let paths: Vec<_> = devices.iter().map(|d| d.path.clone()).collect();
                if session.as_ref().is_some_and(|s| s.paths != paths) {
                    session = None;
                    *device_info.write().await = None;
                }
                if session.is_none() && !devices.is_empty() {
                    generation += 1;
                    match Session::start(&devices, generation, tx.clone()) {
                        Ok(s) => {
                            info!("Tablet attached with {} raw interfaces and exclusive physical input ownership", devices.len());
                            session = Some(s);
                            *device_info.write().await = devices.first().cloned();
                        }
                        Err(e) => warn!("Tablet unavailable (no duplicate input injected): {e:#}"),
                    }
                }
            }
            Some(event) = rx.recv() => {
                match event {
                    HardwareEvent::Disconnected(id) if session.as_ref().is_some_and(|s| s.generation == id) => {
                        session = None;
                        *device_info.write().await = None;
                        info!("Tablet detached; released virtual input and physical grabs");
                    }
                    HardwareEvent::Report(id, data) => {
                        if let Some(s) = session.as_mut().filter(|s| s.generation == id) {
                            let cfg = config.read().await;
                            let result = match parse_report(&data) {
                                Ok(Some(TabletReport::Stylus(p))) => s.tablet.emit_stylus(p, &cfg),
                                Ok(Some(TabletReport::ExpressKeys(p))) => s.tablet.emit_express_keys(p, &cfg),
                                Err(e) => { tracing::debug!("Unsupported packet: {e}"); Ok(()) },
                                _ => Ok(()),
                            };
                            if let Err(e) = result {
                                warn!("Input emission failed: {e}; resetting tablet session");
                                session = None;
                                *device_info.write().await = None;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    drop(session);
    info!("Stopped; released tablet ownership");
    Ok(())
}
