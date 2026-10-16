#!/usr/bin/env python3
"""Release metadata and text consistency checks used by CI."""
import argparse
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--tag')
args = parser.parse_args()
metadata = tomllib.loads((ROOT/'Cargo.toml').read_text())['workspace']['package']
if args.tag and args.tag != 'v'+metadata['version']:
    raise SystemExit(f'Tag {args.tag} does not match Cargo version {metadata["version"]}')
assert 'nirusaki' in metadata['authors']
files = subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'], cwd=ROOT).decode().split('\0')
for name in filter(None, files):
    path = ROOT/name
    if not path.is_file():
        continue
    try:
        text = path.read_text()
    except UnicodeError:
        continue
    if '\u2014' in text:
        raise SystemExit(f'Em dash found in {name}')
print(f'PASS: Kalam {metadata["version"]} attribution, text and tag checks')
