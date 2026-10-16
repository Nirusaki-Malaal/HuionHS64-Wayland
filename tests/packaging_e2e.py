#!/usr/bin/env python3
"""Check the actual archive and staged install/uninstall without touching the host."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
version = tomllib.loads((ROOT/'Cargo.toml').read_text())['workspace']['package']['version']
archive = ROOT/'dist'/f'kalam-{version}-linux-x86_64.tar.gz'
digest, filename = (ROOT/'dist/SHA256SUMS').read_text().split()
assert filename == archive.name
assert hashlib.sha256(archive.read_bytes()).hexdigest() == digest
with tempfile.TemporaryDirectory(prefix='kalam-install-test-') as temporary:
    base = Path(temporary)
    with tarfile.open(archive) as tar:
        tar.extractall(base/'extracted', filter='data')
    bundle = base/'extracted'/archive.name.removesuffix('.tar.gz')
    info = json.loads((bundle/'BUILD-INFO.json').read_text())
    assert info['version'] == version and info['author'] == 'nirusaki'
    assert info['license'] == 'GPL-3.0-or-later'
    assert sorted(path.name for path in bundle.glob('LICENSE*')) == ['LICENSE']
    assert 'GNU GENERAL PUBLIC LICENSE' in (bundle/'LICENSE').read_text()
    assert not (bundle/'research').exists() and not (bundle/'research.md').exists()
    assert not (bundle/'plan.md').exists() and not (bundle/'BUILD_CONTEXT.md').exists()
    spec = importlib.util.spec_from_file_location('installer', bundle/'scripts/manage_install.py')
    installer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(installer)
    stage = base/'stage with spaces'
    config = stage/'home/test/.config/huion-wayland/config.toml'
    config.parent.mkdir(parents=True)
    config.write_text('preserve personal settings')
    command = ['sh', str(bundle/'install.sh'), '--destdir', str(stage), '--no-build', '--no-start']
    subprocess.run(command, cwd=base, check=True)
    subprocess.run(command, cwd=base, check=True)  # upgrades replace existing files
    for key, location in installer.FILES.items():
        target = stage/location.lstrip('/')
        source = bundle/'bin'/key if '/' not in key else bundle/key
        assert target.read_bytes() == source.read_bytes(), target
        if '/' not in key:
            assert os.access(target, os.X_OK), target
    subprocess.run(['sh', str(bundle/'uninstall.sh'), '--destdir', str(stage), '--no-start'], cwd=base, check=True)
    assert all(not (stage/location.lstrip('/')).exists() for location in installer.FILES.values())
    assert config.read_text() == 'preserve personal settings'
    # A malformed bundle must fail before copying a partial installation.
    (bundle/'bin/huion-gui').unlink()
    result = subprocess.run(command, cwd=base, capture_output=True, text=True)
    assert result.returncode != 0 and 'Missing' in result.stderr
    assert all(not (stage/location.lstrip('/')).exists() for location in installer.FILES.values())
print('PASS: checksum, extracted bundle, repeated install, permissions, uninstall and missing-binary failure')
