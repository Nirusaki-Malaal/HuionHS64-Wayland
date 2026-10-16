#!/usr/bin/env python3
"""Capture only supported HS64 HID reports, without changing tablet mode."""
import argparse
import json
import os
from pathlib import Path
import select
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--seconds', type=float, default=30)
parser.add_argument('--output', type=Path, default=Path('huion-capture.json'))
args = parser.parse_args()
if args.seconds <= 0:
    parser.error('--seconds must be positive')
files, devices, summaries, samples = {}, {}, {}, {}
try:
    for node in Path('/sys/class/hidraw').glob('hidraw*'):
        identity = (node/'device/uevent').read_text()
        if 'HID_ID=0003:0000256C:0000006F' not in identity:
            continue
        devices[node.name] = {'identity': identity, 'descriptor': (node/'device/report_descriptor').read_bytes().hex(' ')}
        files[os.open('/dev/'+node.name, os.O_RDONLY | os.O_NONBLOCK)] = node.name
    if not files:
        raise SystemExit('No supported HS64 interfaces found')
    print('Capturing HS64 only. Draw light/firm strokes, lift pen, and press/release buttons.', flush=True)
    deadline = time.monotonic() + args.seconds
    while time.monotonic() < deadline:
        for fd in select.select(list(files), [], [], min(1, max(0, deadline-time.monotonic())))[0]:
            report = os.read(fd, 64)
            if not report:
                continue
            key = files[fd]+':'+report[:1].hex()
            summary = summaries.setdefault(key, {'count': 0})
            summary['count'] += 1
            if report[0] == 0x0a and len(report) >= 8:
                for name, offset in [('x', 2), ('y', 4), ('pressure', 6)]:
                    value = int.from_bytes(report[offset:offset+2], 'little')
                    summary['min_'+name] = min(summary.get('min_'+name, value), value)
                    summary['max_'+name] = max(summary.get('max_'+name, 0), value)
            # Bounded samples; preserve different pressure bands and status changes.
            signature = report.hex(' ')
            values = samples.setdefault(key, [])
            if signature not in values and len(values) < 512:
                values.append(signature)
    args.output.write_text(json.dumps({'devices': devices, 'summaries': summaries, 'samples': samples}, indent=2)+'\n')
    print(json.dumps(summaries, indent=2))
    print('Saved', args.output)
finally:
    for fd in files:
        os.close(fd)
