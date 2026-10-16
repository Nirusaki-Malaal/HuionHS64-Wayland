# Kalam V1.0.0

Kalam is a reverse engineering project made by nirusaki for the Huion HS64 on
native Wayland. It replaces the X11-dependent vendor stack with a Rust userspace
daemon and a GTK4/Libadwaita settings application.

## Included

- Standard-mode HS64 pen pressure, proximity, orientation and active-area handling.
- Exclusive physical input ownership to prevent competing pointer/keyboard events.
- Configurable express buttons with simple presets and balanced key releases.
- Native settings UI with a large pen-test workspace, local canvas rotation,
  and a physical button diagram with position labels.
- CI formatting, lint, test, release build and packaged installer checks.
- Linux x86_64 binaries, SHA256SUMS, build metadata and easy install/uninstall scripts.

## Install

Download `kalam-1.0.0-linux-x86_64.tar.gz` and `SHA256SUMS` from this release.
The binaries are built on Ubuntu 24.04 and require compatible glibc/GTK libraries.

```sh
sha256sum --check SHA256SUMS
tar -xzf kalam-1.0.0-linux-x86_64.tar.gz
cd kalam-1.0.0-linux-x86_64
./install.sh
huion-gui
```

Run as your desktop user; the installer requests sudo for system files and
starts the user service. Use `./install.sh --enable` for autostart and
`./uninstall.sh` to remove installed files while preserving personal settings.
Ubuntu runtime dependencies: `libgtk-4-1 libadwaita-1-0 python3`.

See README.md for source installation, dependencies and keyd exclusions. The
supported hardware is standard-mode HS64 USB `256c:006f`; other Huion products,
vendor mode and compositor output/aspect integration are outside this release.
