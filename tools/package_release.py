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
    parser.add_argument('--arch', choices=['x86_64', 'aarch64', 'arm64'], default=None)
    args = parser.parse_args()
    machine = platform.machine()
    default_arch = 'aarch64' if machine in ('aarch64', 'arm64') else ('x86_64' if machine == 'x86_64' else machine)
    arch = args.arch or default_arch
    if arch == 'arm64':
        arch = 'aarch64'
    if platform.system() != 'Linux' or arch not in ('x86_64', 'aarch64'):
        parser.error(f'This release package supports Linux x86_64 and aarch64 (got {platform.system()} {arch})')
    metadata = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']
    version = metadata['version']
    name = f'kalam-{version}-linux-{arch}'
    args.output.mkdir(parents=True, exist_ok=True)
    epoch = int(output('git', 'show', '-s', '--format=%ct', 'HEAD'))
    with tempfile.TemporaryDirectory(prefix='kalam-package-') as temp:
        bundle = Path(temp) / name
        (bundle / 'bin').mkdir(parents=True)
        target_triple = f'{arch}-unknown-linux-gnu'
        target_dir = ROOT / 'target' / target_triple / 'release'
        if not target_dir.is_dir():
            target_dir = ROOT / 'target' / 'release'
        for binary in ('huion-daemon', 'huion-gui'):
            source = target_dir / binary
            if not source.is_file():
                parser.error(f'Missing {source}; run cargo build --release --locked --workspace')
            target = bundle / 'bin' / binary
            shutil.copy2(source, target)
            subprocess.run(['strip', '--strip-unneeded', str(target)], check=True)
        for relative in ('install.sh', 'uninstall.sh', 'bootstrap.sh', 'scripts/manage_install.py',
                         'README.md', 'docs/DOCUMENTATION.md', 'docs/RELEASE_NOTES.md', 'LICENSE',
                         'data/70-huion-wayland.rules', 'data/huion-wayland.service',
                         'data/io.github.nirusaki.HuionWayland.desktop', 'data/kalam-uinput.conf',
                         'docs/benchmarks.json', 'tools/benchmark.py',
                         'docs/images/settings.png', 'docs/images/buttons.png',
                         'docs/images/pen-workspace.png', 'docs/images/pen-portrait.png'):
            source = ROOT / relative
            target = bundle / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        (bundle / 'BUILD-INFO.json').write_text(json.dumps({
            'project': 'Kalam', 'version': version, 'author': 'nirusaki',
            'license': metadata['license'],
            'description': 'A reverse engineering project for the Huion HS64 on native Wayland',
            'commit': output('git', 'rev-parse', 'HEAD'),
            'target': target_triple,
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
