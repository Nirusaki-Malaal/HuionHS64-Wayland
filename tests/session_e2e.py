#!/usr/bin/env python3
"""Exercise the running daemon's D-Bus contract without changing user settings."""
import ast
import json
import subprocess

PREFIX = ['gdbus', 'call', '--session', '--dest', 'org.wayland.HuionTablet',
          '--object-path', '/org/wayland/HuionTablet', '--method']

def call(method, *args):
    return subprocess.check_output(PREFIX + ['org.wayland.HuionTablet.' + method] + list(args), text=True).strip()

def get(method):
    return json.loads(ast.literal_eval(call(method))[0])

original = get('GetConfig')
try:
    assert call('SetConfig', json.dumps(original)) == '(true,)'
    assert get('GetConfig') == original
    bad = dict(original, active_area=dict(original['active_area'], width=0))
    assert call('SetConfig', json.dumps(bad)) == '(false,)'
    bad = dict(original, express_keys=[{'type':'Key','value':'KEY_NOT_A_REAL_KEY'}] * 4)
    assert call('SetConfig', json.dumps(bad)) == '(false,)'
    bad = dict(original, target_output='unsupported-output')
    assert call('SetConfig', json.dumps(bad)) == '(false,)'
    assert get('GetConfig') == original
    device = get('GetDeviceInfo')
    displays = get('GetDisplays')
    assert isinstance(device.get('connected'), bool)
    assert isinstance(displays, list)
    print('PASS: D-Bus roundtrip, rejected invalid area/key/output, unchanged settings, status and display schemas')
    print('Hardware:', json.dumps(device))
finally:
    assert call('SetConfig', json.dumps(original)) == '(true,)'
