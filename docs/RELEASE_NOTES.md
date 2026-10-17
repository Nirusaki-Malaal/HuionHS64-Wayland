# Kalam V1.1.0

Kalam V1.1.0 brings official Android support alongside the native Linux Wayland stack for the Huion HS64 tablet. It introduces an unrooted Material You (Material 3) Android app with direct USB OTG communication, real-time Bezier curve tuning, 4-way rotation, and configurable express keys.

## Included in V1.1.0

- Native Android Support (Unrooted): Direct USB OTG communication via Android UsbManager and custom HID parser, compatible with Android 8.0 through Android 17+.
- Material You Design: Jetpack Compose Material 3 interface with dynamic color theming, real-time pressure gauge, and responsive layout.
- Dynamic Rotation & Canvas: Full 0 deg, 90 deg, 180 deg, and 270 deg rotation support with dynamic aspect ratio scaling and an express key indicator strip.
- Express Key Remapping: Interactive shortcut dialog with presets (Undo, Redo, Brush, Eraser, Zoom, Pan) and custom mappings persisted via Jetpack DataStore.
- Automated CI/CD: GitHub Actions workflows building and validating both Linux release bundles and installable Android APKs.
- Standard Linux Wayland Stack: The complete Rust daemon, uinput emulation, and GTK4/Libadwaita settings app.

## Install on Linux

Download `kalam-1.1.0-linux-x86_64.tar.gz` (for Intel/AMD x86_64) or `kalam-1.1.0-linux-aarch64.tar.gz` (for ARM64/aarch64) and `SHA256SUMS` from this release.

```sh
sha256sum --check SHA256SUMS
tar -xzf kalam-1.1.0-linux-x86_64.tar.gz  # or kalam-1.1.0-linux-aarch64.tar.gz
cd kalam-1.1.0-linux-*
./install.sh
huion-gui
```

## Install on Android

Download `kalam-android-v1.1.0.apk` or `kalam-android-arm64-v1.1.0.apk` from the release assets and install it directly on your Android phone or tablet. Connect the Huion HS64 via a USB OTG adapter and grant USB permission when prompted.

