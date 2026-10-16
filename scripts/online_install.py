#!/usr/bin/env python3
"""Detect a Linux desktop, install native build dependencies, then install Kalam."""
import argparse
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
# Source builds use the host's libc and architecture, not a foreign binary ABI.
PACKAGES = {
    'apt-get': ['build-essential', 'pkg-config', 'libgtk-4-dev', 'libadwaita-1-dev', 'ca-certificates', 'udev'],
    'dnf': ['gcc', 'gcc-c++', 'make', 'pkgconf-pkg-config', 'gtk4-devel', 'libadwaita-devel', 'ca-certificates', 'systemd-udev'],
    'pacman': ['base-devel', 'pkgconf', 'gtk4', 'libadwaita', 'ca-certificates'],
    'zypper': ['gcc', 'gcc-c++', 'make', 'pkg-config', 'gtk4-devel', 'libadwaita-devel', 'ca-certificates'],
    'apk': ['build-base', 'pkgconf', 'gtk4.0-dev', 'libadwaita-dev', 'linux-headers', 'ca-certificates', 'eudev', 'rust', 'cargo'],
    'xbps-install': ['base-devel', 'pkg-config', 'gtk4-devel', 'libadwaita-devel', 'ca-certificates'],
    'emerge': ['sys-devel/gcc', 'virtual/pkgconfig', 'gui-libs/gtk:4', 'gui-libs/libadwaita', 'app-misc/ca-certificates'],
}
PREFIXES = {
    'apt-get': ['apt-get', 'install', '-y'], 'dnf': ['dnf', 'install', '-y'],
    'pacman': ['pacman', '-S', '--needed', '--noconfirm'],
    'zypper': ['zypper', '--non-interactive', 'install'], 'apk': ['apk', 'add'],
    'xbps-install': ['xbps-install', '-Sy'], 'emerge': ['emerge', '--noreplace'],
}


def os_release(path=Path('/etc/os-release')):
    values = {}
    for line in path.read_text().splitlines():
        if '=' in line and not line.lstrip().startswith('#'):
            key, value = line.split('=', 1)
            parts = shlex.split(value)
            values[key] = ' '.join(parts)
    return values


def wayland_session(env):
    session = env.get('XDG_SESSION_TYPE', '').lower()
    return session == 'wayland' or (session != 'x11' and bool(env.get('WAYLAND_DISPLAY')))


def package_manager(info, which=shutil.which):
    families = (info.get('ID', '') + ' ' + info.get('ID_LIKE', '')).split()
    mapping = {'ubuntu':'apt-get', 'debian':'apt-get', 'linuxmint':'apt-get',
               'arch':'pacman', 'manjaro':'pacman', 'artix':'pacman',
               'fedora':'dnf', 'rhel':'dnf', 'centos':'dnf',
               'suse':'zypper', 'opensuse':'zypper', 'opensuse-tumbleweed':'zypper', 'opensuse-leap':'zypper',
               'alpine':'apk', 'void':'xbps-install', 'gentoo':'emerge'}
    for family in families:
        manager = mapping.get(family)
        if manager and which(manager):
            return manager
    return next((manager for manager in PACKAGES if which(manager)), None)


def rust_ready():
    try:
        result = subprocess.check_output(['rustc', '--version'], text=True, stderr=subprocess.DEVNULL)
        version = re.search(r'rustc (\d+)\.(\d+)', result)
        return bool(version and tuple(map(int, version.groups())) >= (1, 89) and shutil.which('cargo'))
    except (OSError, subprocess.CalledProcessError):
        return False


def privileged(command):
    escalation = next((name for name in ('sudo', 'doas') if shutil.which(name)), None)
    if not escalation:
        raise RuntimeError('Install sudo or doas, or prepare dependencies manually; see documentation.md.')
    subprocess.run([escalation, *command], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--uninstall', action='store_true', help='Remove Kalam without building it')
    parser.add_argument('--dry-run', action='store_true', help='Print the plan without installing packages or files')
    parser.add_argument('--no-deps', action='store_true', help='Use dependencies and Rust already installed')
    parser.add_argument('--enable', action='store_true', help='Enable login autostart')
    parser.add_argument('--no-start', action='store_true', help='Install without starting the daemon')
    args = parser.parse_args()
    if platform.system() != 'Linux' or os.geteuid() == 0:
        parser.error('Run from your Linux desktop terminal as your normal user, without sudo')
    if not args.uninstall and not wayland_session(os.environ):
        parser.error('No Wayland session detected. Log into Wayland and retry')
    if not args.no_start and not os.environ.get('DBUS_SESSION_BUS_ADDRESS'):
        parser.error('A desktop session bus is required. Open a terminal in your desktop')
    if args.enable and args.no_start:
        parser.error('--enable and --no-start cannot be combined')
    if args.uninstall:
        if args.enable:
            parser.error('--enable does not apply to uninstall')
        if args.dry_run:
            print('Would stop Kalam and remove installed files; personal settings are kept')
            return
        forwarded = ['--no-start'] if args.no_start else []
        subprocess.run([sys.executable, str(ROOT/'scripts/manage_install.py'), 'uninstall', *forwarded], check=True)
        return
    info = os_release()
    manager = package_manager(info)
    immutable = info.get('ID') == 'nixos' or Path('/run/ostree-booted').exists()
    print('Desktop: Wayland | Distribution:', info.get('PRETTY_NAME', info.get('ID', 'unknown')), flush=True)
    print('Build: native source for', platform.machine(), '| Package manager:', manager or 'manual', flush=True)
    if args.dry_run:
        if manager:
            print('Dependencies:', shlex.join(PREFIXES[manager] + PACKAGES[manager]))
        print('Rust >= 1.89:', 'available' if rust_ready() else 'will install with rustup as your user')
        print('Then: compile Kalam and install with desktop service management')
        if immutable:
            print('Immutable/NixOS system: prepare a native build environment first and use --no-deps; see documentation.md')
        return
    if not args.no_deps:
        if immutable or not manager:
            raise RuntimeError('Prepare native GTK4/Libadwaita development dependencies first, then use --no-deps. See documentation.md')
        if manager == 'apt-get':
            privileged(['apt-get', 'update'])
        privileged(PREFIXES[manager] + PACKAGES[manager])
    if not rust_ready():
        if args.no_deps:
            raise RuntimeError('Rust 1.89 or newer and Cargo are required')
        print('Installing Rust for your user...', flush=True)
        with tempfile.TemporaryDirectory(prefix='kalam-rust-') as directory:
            script = Path(directory)/'rustup.sh'
            request = urllib.request.Request('https://sh.rustup.rs', headers={'User-Agent':'Kalam installer'})
            with urllib.request.urlopen(request, timeout=60) as response:
                script.write_bytes(response.read())
            subprocess.run(['sh', str(script), '-y', '--profile', 'minimal', '--default-toolchain', 'stable'], check=True)
        os.environ['PATH'] = str(Path.home()/'.cargo/bin') + os.pathsep + os.environ['PATH']
    if not rust_ready():
        raise RuntimeError('Rust installation did not provide a usable compiler; see documentation.md')
    if not shutil.which('udevadm'):
        raise RuntimeError('udev/eudev is required. Enable it for your desktop, then retry')
    # Fail before copying system files if the distro's GTK versions are too old.
    subprocess.run(['pkg-config', '--atleast-version=1.2', 'libadwaita-1'], check=True)
    subprocess.run(['pkg-config', '--exists', 'gtk4'], check=True)
    subprocess.run(['cargo', 'build', '--release', '--locked', '--workspace'], cwd=ROOT, check=True)
    forwarded = [flag for flag, selected in [('--enable', args.enable), ('--no-start', args.no_start)] if selected]
    subprocess.run([sys.executable, str(ROOT/'scripts/manage_install.py'), 'install', '--no-build', *forwarded], check=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print('Kalam installation stopped:', error, file=sys.stderr)
        sys.exit(1)
