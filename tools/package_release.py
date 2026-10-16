#!/usr/bin/env python3
"""Create the Kalam binary bundle and checksum from an existing release build."""
import argparse
import gzip
import hashlib
import json
import platform
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def output(*command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'dist')
    args = parser.parse_args()
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        parser.error('This release package supports Linux x86_64')
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    name = f'kalam-{version}-linux-x86_64'
    args.output.mkdir(parents=True, exist_ok=True)
    epoch = int(output('git', 'show', '-s', '--format=%ct', 'HEAD'))
    with tempfile.TemporaryDirectory(prefix='kalam-package-') as temp:
        bundle = Path(temp) / name
        (bundle / 'bin').mkdir(parents=True)
        for binary in ('huion-daemon', 'huion-gui'):
            source = ROOT / 'target/release' / binary
            if not source.is_file():
                parser.error(f'Missing {source}; run cargo build --release --locked --workspace')
            target = bundle / 'bin' / binary
            shutil.copy2(source, target)
            subprocess.run(['strip', '--strip-unneeded', str(target)], check=True)
        for relative in ('install.sh', 'uninstall.sh', 'scripts/manage_install.py',
                         'README.md', 'documentation.md', 'RELEASE_NOTES.md', 'LICENSE-MIT', 'LICENSE-APACHE',
                         'data/70-huion-wayland.rules', 'data/huion-wayland.service',
                         'data/io.github.nirusaki.HuionWayland.desktop', 'data/kalam-uinput.conf',
                         'docs/images/settings.png', 'docs/images/buttons.png',
                         'docs/images/pen-workspace.png', 'docs/images/pen-portrait.png'):
            source = ROOT / relative
            target = bundle / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        (bundle / 'BUILD-INFO.json').write_text(json.dumps({
            'project': 'Kalam', 'version': version, 'author': 'nirusaki',
            'description': 'A reverse engineering project for the Huion HS64 on native Wayland',
            'commit': output('git', 'rev-parse', 'HEAD'),
            'target': 'x86_64-unknown-linux-gnu',
            'build_system': platform.freedesktop_os_release().get('PRETTY_NAME'),
            'glibc': platform.libc_ver(),
            'rustc': output('rustc', '--version'),
            'gtk_and_adwaita': output('pkg-config', '--modversion', 'gtk4', 'libadwaita-1').splitlines(),
        }, indent=2) + '\n')
        archive = args.output / (name + '.tar.gz')
        with archive.open('wb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as tar:
                for path in [bundle, *sorted(bundle.rglob('*'))]:
                    info = tar.gettarinfo(str(path), arcname=str(path.relative_to(bundle.parent)))
                    info.uid = info.gid = 0
                    info.uname = info.gname = ''
                    info.mtime = epoch
                    if path.is_file():
                        with path.open('rb') as data:
                            tar.addfile(info, data)
                    else:
                        tar.addfile(info)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (args.output / 'SHA256SUMS').write_text(f'{digest}  {archive.name}\n')
        print(archive)
        print(args.output / 'SHA256SUMS')


if __name__ == '__main__':
    main()
