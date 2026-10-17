# Kalam

**A reverse engineering project by nirusaki.** A native Wayland driver and Material You Android client for the **Huion HS64**.

## Supported Platforms

- **Linux (Wayland)**: Native Rust userspace daemon and GTK4/Libadwaita settings application (`x86_64` and `aarch64` / ARM64).
- **Android (Non-Rooted)**: Jetpack Compose Material You application running on Android 8.0 to Android 17+ via direct USB OTG.

## Install on Linux (Wayland)

Open a terminal in your Wayland desktop and paste this, **without sudo**:

```sh
curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/HuionHS64-Wayland/main/bootstrap.sh | sh -s -- --enable
```

The installer detects your distro, installs dependencies, builds the app and starts the driver at login. It asks for your password when needed. [Special setups and uninstalling](docs/DOCUMENTATION.md#online-installer).

Pre-built binaries for `x86_64` and `aarch64` are also available in [Releases](https://github.com/Nirusaki-Malaal/HuionHS64-Wayland/releases).

## Install on Android

1. Download `kalam-android-v1.1.0.apk` (or `kalam-android-arm64-v1.1.0.apk`) from [Releases](https://github.com/Nirusaki-Malaal/HuionHS64-Wayland/releases).
2. Install the APK on your device (no root required).
3. Connect your Huion HS64 tablet using a USB OTG adapter and grant USB permissions when prompted.

## Features

- **Stylus Pressure**: 8192 levels with real-time configurable cubic Bezier response curves (Linear, Soft, Firm, Custom).
- **4-Way Tablet Rotation**: 0 deg (Landscape), 90 deg (Portrait), 180 deg (Inverted Landscape), and 270 deg (Inverted Portrait) with automatic coordinate transformation.
- **Express Keys**: 4 configurable hardware buttons with quick action presets (Undo, Redo, Brush, Eraser, Zoom, Pan) and visual HUD feedback.
- **Test Canvas**: Integrated scratchpad to test pressure sensitivity, stylus hover/proximity, and express keys.

## AI usage disclosure

Reverse engineering and development were AI-assisted using **GEMINI 3.8 Flash**, **Muse Spark 1.3**, and **GPT-6.1 Sol Light**.

## License

[GPL-3.0-or-later](LICENSE). Want to know more? Read the [documentation](docs/DOCUMENTATION.md).

## Screenshots

<p><img src="docs/images/settings.png" width="32%" alt="Tablet settings"> <img src="docs/images/pen-workspace.png" width="32%" alt="Pen workspace"> <img src="docs/images/buttons.png" width="32%" alt="Button layout and shortcuts"></p>

## Performance

| Component | Memory (PSS) | Idle CPU |
| --- | ---: | ---: |
| Driver | 2.74 MiB | 0.17% |
| Settings window | 49.42 MiB | 0.77% |

30-second observation on Arch Linux/Wayland, with the settings window open. Results vary. [Method and raw measurements](docs/DOCUMENTATION.md#resource-measurements).

