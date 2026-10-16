#!/bin/sh
# Online source installer. Run as your desktop user, not with sudo.
set -eu
if [ "${1:-}" = '--help' ]; then
    echo 'Install Kalam: curl -fsSL https://raw.githubusercontent.com/Nirusaki-Malaal/HuionHS64-Wayland/main/bootstrap.sh | sh'
    echo 'Options: --dry-run, --no-deps, --enable, --no-start, --uninstall'
    exit 0
fi
[ "$(uname -s)" = Linux ] || { echo 'Kalam requires Linux.' >&2; exit 1; }
[ "$(id -u)" != 0 ] || { echo 'Run from your desktop terminal without sudo.' >&2; exit 1; }
uninstall=false
for option in "$@"; do [ "$option" != --uninstall ] || uninstall=true; done
if [ "$uninstall" = false ]; then
case "${XDG_SESSION_TYPE:-}" in
    wayland) ;;
    x11) echo 'Please log into a Wayland desktop and run this command there.' >&2; exit 1 ;;
    *) [ -n "${WAYLAND_DISPLAY:-}" ] || { echo 'No Wayland session detected. Open a terminal in your Wayland desktop.' >&2; exit 1; } ;;
esac
fi
command -v curl >/dev/null || { echo 'Install curl first.' >&2; exit 1; }
command -v tar >/dev/null || { echo 'Install tar first.' >&2; exit 1; }
if ! command -v python3 >/dev/null; then
    if command -v sudo >/dev/null; then
        elevate=sudo
    elif command -v doas >/dev/null; then
        elevate=doas
    else
        echo 'Install Python 3 and sudo or doas first.' >&2
        exit 1
    fi
    if command -v apt-get >/dev/null; then "$elevate" apt-get update; "$elevate" apt-get install -y python3
    elif command -v dnf >/dev/null; then "$elevate" dnf install -y python3
    elif command -v pacman >/dev/null; then "$elevate" pacman -S --needed --noconfirm python
    elif command -v zypper >/dev/null; then "$elevate" zypper --non-interactive install python3
    elif command -v apk >/dev/null; then "$elevate" apk add python3
    elif command -v xbps-install >/dev/null; then "$elevate" xbps-install -Sy python3
    elif command -v emerge >/dev/null; then "$elevate" emerge --noreplace dev-lang/python
    else echo 'Install Python 3 using your package manager, then retry.' >&2; exit 1
    fi
fi
workdir=$(mktemp -d)
cleanup() { python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1], ignore_errors=True)' "$workdir"; }
trap cleanup 0
trap 'exit 130' INT
trap 'exit 143' TERM
echo 'Downloading Kalam source...'
curl -fL --retry 3 --connect-timeout 15 --max-time 120 \
    https://github.com/Nirusaki-Malaal/HuionHS64-Wayland/archive/refs/heads/main.tar.gz -o "$workdir/source.tar.gz"
tar -xzf "$workdir/source.tar.gz" -C "$workdir"
python3 "$workdir/HuionHS64-Wayland-main/scripts/online_install.py" "$@"
