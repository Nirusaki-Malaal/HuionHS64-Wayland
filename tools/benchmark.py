#!/usr/bin/env python3
"""Measure already-running Kalam processes. Does not synthesize input or restart them."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import time


def sample(pid):
    base = Path('/proc')/str(pid)
    fields = (base/'stat').read_text().rsplit(')', 1)[1].split()
    cpu = (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK')
    memory = {}
    for line in (base/'smaps_rollup').read_text().splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[2] == 'kB':
            memory[parts[0].rstrip(':')] = int(parts[1])
    return {'cpu_seconds':cpu, 'rss_kib':memory['Rss'], 'pss_kib':memory['Pss']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--seconds', type=int, default=30)
    parser.add_argument('--output', type=Path, default=Path('docs/benchmarks.json'))
    args = parser.parse_args()
    if args.seconds < 2:
        parser.error('--seconds must be at least 2')
    pids = {}
    for name in ['huion-daemon', 'huion-gui']:
        output = subprocess.check_output(['pgrep', '-u', str(os.getuid()), '-x', name], text=True)
        candidates = [int(pid) for pid in output.split()]
        if len(candidates) != 1:
            raise SystemExit(f'Expected one running {name}; found {candidates}')
        pids[name] = candidates[0]
    samples = {name:[] for name in pids}
    start = time.monotonic()
    for index in range(args.seconds + 1):
        for name, pid in pids.items():
            samples[name].append(sample(pid))
        if index < args.seconds:
            time.sleep(max(0, start+index+1-time.monotonic()))
    elapsed = time.monotonic()-start
    result = {'measured_at_utc':datetime.now(timezone.utc).isoformat(),
              'workload':'Idle observation, driver running and settings window open; no scripted tablet input',
              'sample_interval_seconds':1, 'duration_seconds':round(elapsed,3),
              'distribution':platform.freedesktop_os_release().get('PRETTY_NAME'),
              'kernel':platform.release(), 'session':os.environ.get('XDG_SESSION_TYPE'),
              'cpu':next(line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),
              'processes':{}}
    for name, values in samples.items():
        pid = pids[name]
        result['processes'][name] = {
            'pid':pid, 'samples':len(values),
            'binary_sha256':hashlib.sha256((Path('/proc')/str(pid)/'exe').read_bytes()).hexdigest(),
            'rss_median_mib':round(statistics.median(v['rss_kib'] for v in values)/1024,2),
            'rss_max_mib':round(max(v['rss_kib'] for v in values)/1024,2),
            'pss_median_mib':round(statistics.median(v['pss_kib'] for v in values)/1024,2),
            'cpu_percent_one_core':round(100*(values[-1]['cpu_seconds']-values[0]['cpu_seconds'])/elapsed,3),
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))


if __name__ == '__main__':
    main()
