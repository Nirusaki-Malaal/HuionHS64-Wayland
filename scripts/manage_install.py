#!/usr/bin/env python3
"""Install Kalam from a source checkout or extracted release. No Cargo runs as root."""
import argparse
import os
from pathlib import Path
import shutil
import signal
import time
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FILES = {
    'huion-daemon': '/usr/local/bin/huion-daemon',
    'huion-gui': '/usr/local/bin/huion-gui',
    'data/70-huion-wayland.rules': '/etc/udev/rules.d/70-huion-wayland.rules',
    'data/huion-wayland.service': '/usr/local/lib/systemd/user/huion-wayland.service',
    'data/io.github.nirusaki.HuionWayland.desktop': '/usr/local/share/applications/io.github.nirusaki.HuionWayland.desktop',
    'data/kalam-uinput.conf': '/etc/modules-load.d/kalam-uinput.conf',
}


def service(*args):
    subprocess.run(['systemctl', '--user', *args], check=True)


def systemd_user_available():
    if not shutil.which('systemctl') or not Path('/run/systemd/system').exists():
        return False
    try:
        return subprocess.run(['systemctl', '--user', 'show-environment'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def stop_standalone():
    # Stop only our user's installed daemon, not other drivers or other users.
    for process in Path('/proc').glob('[0-9]*'):
        try:
            if process.stat().st_uid != os.getuid():
                continue
            executable = os.readlink(process/'exe').removesuffix(' (deleted)')
            if executable != '/usr/local/bin/huion-daemon':
                continue
            os.kill(int(process.name), signal.SIGTERM)
            deadline = time.monotonic()+3
            while (process/'exe').exists() and time.monotonic()<deadline:
                time.sleep(.05)
            if (process/'exe').exists():
                raise RuntimeError('The old daemon did not stop; close it and retry')
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue


def standalone_service(operation, enable=False):
    config_home = Path(os.environ.get('XDG_CONFIG_HOME') or Path.home()/'.config')
    autostart = config_home/'autostart/io.github.nirusaki.HuionDaemon.desktop'
    stop_standalone()
    if operation == 'uninstall':
        autostart.unlink(missing_ok=True)
        return
    if enable:
        autostart.parent.mkdir(parents=True, exist_ok=True)
        autostart.write_text('[Desktop Entry]\nType=Application\nName=Kalam Tablet Driver\nExec=/usr/local/bin/huion-daemon\nTerminal=false\nX-GNOME-Autostart-enabled=true\n')
    cache = Path(os.environ.get('XDG_CACHE_HOME') or Path.home()/'.cache')/'kalam'
    cache.mkdir(parents=True, exist_ok=True)
    with (cache/'daemon.log').open('ab') as log:
        process = subprocess.Popen(['/usr/local/bin/huion-daemon'], stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
    time.sleep(.3)
    if process.poll() is not None:
        raise RuntimeError(f'Daemon failed to start; see {cache / "daemon.log"}')


def copy_atomic(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix='.' + target.name + '-', dir=target.parent)
    temporary = Path(name)
    try:
        with os.fdopen(fd, 'wb') as output, source.open('rb') as source_file:
            shutil.copyfileobj(source_file, output)
        temporary.chmod(0o755 if target.parent == Path('/usr/local/bin') or target.name in ('huion-daemon', 'huion-gui') else 0o644)
        temporary.replace(target)
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['install', 'uninstall'])
    parser.add_argument('--no-build', action='store_true', help='Use existing target/release binaries in a source checkout')
    parser.add_argument('--no-start', action='store_true', help='Skip user service management')
    parser.add_argument('--enable', action='store_true', help='Enable service autostart when installing')
    parser.add_argument('--destdir', type=Path, help='Stage files here, without sudo, module/udev changes or service management')
    parser.add_argument('--files-only', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if sys.platform != 'linux':
        parser.error('Kalam requires Linux')
    if args.enable and (args.operation != 'install' or args.no_start or args.destdir):
        parser.error('--enable requires a normal install with service management')
    if args.destdir:
        args.destdir = args.destdir.expanduser().absolute()
        if args.destdir.resolve() == Path('/'):
            parser.error('--destdir must be a staging directory, not /')
    manage_service = not args.destdir and not args.files_only and not args.no_start and os.geteuid() != 0
    if manage_service and not os.environ.get('DBUS_SESSION_BUS_ADDRESS'):
        parser.error('Run in your desktop session, or use --no-start')

    use_systemd = systemd_user_available() if manage_service else False
    binary_dir = ROOT / ('bin' if (ROOT / 'bin').is_dir() else 'target/release')
    if args.operation == 'install':
        if binary_dir == ROOT / 'target/release' and not args.no_build and not args.files_only:
            if os.geteuid() == 0:
                parser.error('Build as your desktop user; run ./install.sh without sudo, or build first and use --no-build')
            subprocess.run(['cargo', 'build', '--release', '--locked', '--workspace'], cwd=ROOT, check=True)
        sources = {key: binary_dir / key if '/' not in key else ROOT / key for key in FILES}
        for source in sources.values():
            if not source.is_file():
                parser.error(f'Missing {source}; build first or extract the complete release archive')
    elif manage_service and use_systemd:
        # Stop before removing binaries, and remove any autostart symlink.
        subprocess.run(['systemctl', '--user', 'disable', '--now', 'huion-wayland.service'], check=False)

    if args.operation == 'uninstall' and manage_service and not use_systemd:
        standalone_service('uninstall')

    if not args.destdir and os.geteuid() != 0:
        escalation = next((name for name in ('sudo', 'doas') if shutil.which(name)), None)
        if not escalation:
            raise RuntimeError('Install sudo or doas to copy system files')
        command = [escalation, sys.executable, str(Path(__file__).resolve()), args.operation, '--files-only', '--no-build', '--no-start']
        subprocess.run(command, check=True)
    else:
        if not args.destdir and args.operation == 'install' and not Path('/dev/uinput').exists():
            subprocess.run(['modprobe', 'uinput'], check=True)
        for key, installed_path in FILES.items():
            target = (args.destdir / installed_path.lstrip('/')) if args.destdir else Path(installed_path)
            if args.operation == 'install':
                copy_atomic(sources[key], target)
            else:
                target.unlink(missing_ok=True)
        if not args.destdir:
            subprocess.run(['udevadm', 'control', '--reload'], check=True)
            # Apply permissions to devices already plugged in, including uinput
            # created before the new rules were copied. Avoid unrelated devices.
            nodes = [*Path('/sys/class/input').glob('event*'),
                     *Path('/sys/class/hidraw').glob('hidraw*')]
            for node in nodes:
                if '256C:006F' in str(node.resolve()).upper():
                    subprocess.run(['udevadm', 'trigger', '--action=change', str(node)], check=True)
            uinput = Path('/sys/class/misc/uinput')
            if uinput.exists():
                subprocess.run(['udevadm', 'trigger', '--action=change', str(uinput)], check=True)
            subprocess.run(['udevadm', 'settle'], check=True)

    if manage_service and not use_systemd:
        if args.operation == 'install':
            standalone_service('install', args.enable)
    elif manage_service:
        service('daemon-reload')
        if args.operation == 'install':
            if args.enable:
                service('enable', 'huion-wayland.service')
            service('restart', 'huion-wayland.service')
    if not args.files_only:
        if args.destdir:
            print(f'{args.operation.capitalize()} staging complete: {args.destdir}')
        elif args.operation == 'install':
            print('Kalam installed. Open settings with: huion-gui')
            if not manage_service:
                print('As your desktop user: systemctl --user daemon-reload && systemctl --user restart huion-wayland.service')
            print('If using keyd, exclude -256c:006f in its [ids] section. See docs/documentation.md.')
        else:
            print('Kalam uninstalled. Your personal configuration was preserved.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'Installation error: {error}', file=sys.stderr)
        sys.exit(1)
