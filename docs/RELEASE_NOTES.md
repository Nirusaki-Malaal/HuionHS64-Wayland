# Kalam V1.1.0

Kalam V1.1.0 delivers multi-architecture binary releases (`x86_64` and `aarch64` / ARM64) alongside automated CI/CD validation for the native Linux Wayland driver stack for the Huion HS64 tablet.

## Included in V1.1.0

- Multi-Architecture Support: Native release packages and validation for `x86_64` and `aarch64` (ARM64).
- Automated CI/CD Pipeline: GitHub Actions release pipeline building, testing, and packaging multi-arch Linux tarballs with SHA256 checksums.
- Standard Linux Wayland Stack: Complete Rust daemon, uinput virtual tablet emulation, and GTK4/Libadwaita settings application.
- 4-Way Rotation & Dynamic Mapping: 0 deg, 90 deg, 180 deg, and 270 deg orientation support with aspect ratio scaling.
- Express Key & Pressure Tuning: Configurable cubic Bezier response curve and hardware button remapping.

## Install

Download `kalam-1.1.0-linux-x86_64.tar.gz` (for Intel/AMD x86_64) or `kalam-1.1.0-linux-aarch64.tar.gz` (for ARM64/aarch64) and `SHA256SUMS` from this release.

```sh
sha256sum --check SHA256SUMS
tar -xzf kalam-1.1.0-linux-x86_64.tar.gz  # or kalam-1.1.0-linux-aarch64.tar.gz
cd kalam-1.1.0-linux-*
./install.sh
huion-gui
```

