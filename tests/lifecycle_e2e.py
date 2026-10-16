#!/usr/bin/env python3
"""Live service ownership regression; briefly stops and always restarts the daemon."""
import ast
import json
import errno
import fcntl
import os
from pathlib import Path
import subprocess
import time


def virtual_nodes():
    return [node for node in Path('/sys/class/input').glob('event*')
            if (node/'device/name').read_text().startswith('Huion HS64 Wayland')]


def physical_nodes():
    return [node for node in Path('/sys/class/input').glob('event*') if '256C:006F' in str(node.resolve())]


def verify_grab(node, available):
    fd = os.open('/dev/input/' + node.name, os.O_RDONLY | os.O_NONBLOCK)
    try:
        try:
            fcntl.ioctl(fd, 0x40044590, 1)  # EVIOCGRAB from linux/input.h
        except OSError as error:
            assert not available and error.errno == errno.EBUSY, (node, error)
        else:
            assert available, f'{node} is not exclusively owned by the daemon'
    finally:
        os.close(fd)


try:
    started = time.monotonic()
    subprocess.run(['systemctl', '--user', 'stop', 'huion-wayland.service'], check=True)
    assert time.monotonic()-started < 3, 'Shutdown hung on a blocking HID read'
    assert not virtual_nodes(), 'Virtual devices survived daemon termination'
    physical = physical_nodes()
    assert physical, 'HS64 required for live lifecycle test'
    for node in physical:
        verify_grab(node, True)
        props = subprocess.check_output(['udevadm', 'info', '-q', 'property', '-n', '/dev/input/'+node.name], text=True)
        assert 'LIBINPUT_IGNORE_DEVICE=1' not in props, 'Old ignore rule prevents kernel fallback'
finally:
    subprocess.run(['systemctl', '--user', 'start', 'huion-wayland.service'], check=True)

end = time.monotonic()+5
# UI_DEV_CREATE makes nodes visible slightly before physical grabs complete.
# Wait for the daemon's published readiness, not merely the node count.
while time.monotonic() < end:
    reply = subprocess.run(['gdbus', 'call', '--session', '--dest', 'org.wayland.HuionTablet',
                            '--object-path', '/org/wayland/HuionTablet', '--method',
                            'org.wayland.HuionTablet.GetDeviceInfo'], capture_output=True, text=True)
    if reply.returncode == 0 and json.loads(ast.literal_eval(reply.stdout)[0]).get('connected'):
        break
    time.sleep(.05)
else:
    raise AssertionError('Daemon did not acquire the tablet within five seconds')
assert len(virtual_nodes()) == 2
for node in physical_nodes():
    verify_grab(node, False)
result = subprocess.run(['/usr/local/bin/huion-daemon'], capture_output=True, text=True, timeout=3)
assert result.returncode != 0 and 'Another Huion daemon' in result.stderr
assert len(virtual_nodes()) == 2
print('PASS: prompt SIGTERM, physical fallback, exclusive reacquisition, singleton and exactly two replacement devices')
