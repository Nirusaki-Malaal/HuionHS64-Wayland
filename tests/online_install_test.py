#!/usr/bin/env python3
"""Platform/installer tests never install packages or alter host services."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import tarfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

online = load('online', ROOT/'scripts/online_install.py')
manage = load('manage', ROOT/'scripts/manage_install.py')


class InstallerTests(unittest.TestCase):
    def test_wayland_and_sway_from_tty(self):
        for env in [{'XDG_SESSION_TYPE':'wayland'}, {'WAYLAND_DISPLAY':'wayland-1'}, {'XDG_SESSION_TYPE':'tty','WAYLAND_DISPLAY':'wayland-1'}]:
            self.assertTrue(online.wayland_session(env))
        for env in [{}, {'DISPLAY':':0'}, {'XDG_SESSION_TYPE':'x11','WAYLAND_DISPLAY':'stale'}]:
            self.assertFalse(online.wayland_session(env))

    def test_distro_families_and_derivatives(self):
        for info, expected in [({'ID':'ubuntu'},'apt-get'), ({'ID':'pop','ID_LIKE':'ubuntu debian'},'apt-get'),
                               ({'ID':'endeavouros','ID_LIKE':'arch'},'pacman'), ({'ID':'rocky','ID_LIKE':'rhel fedora'},'dnf'),
                               ({'ID':'opensuse-tumbleweed'},'zypper'), ({'ID':'alpine'},'apk'),
                               ({'ID':'void'},'xbps-install'), ({'ID':'gentoo'},'emerge')]:
            self.assertEqual(online.package_manager(info, lambda name: name==expected), expected)

    def test_unknown_distro_can_use_existing_package_manager(self):
        self.assertEqual(online.package_manager({'ID':'custom'}, lambda name:name=='dnf'), 'dnf')
        self.assertIsNone(online.package_manager({'ID':'custom'}, lambda name:False))

    def test_os_release_does_not_execute_shell(self):
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'os-release'
            path.write_text('ID=debian\nPRETTY_NAME="Friendly Linux"\nBAD="$(touch do-not-create)"\n')
            info=online.os_release(path)
            self.assertEqual(info['PRETTY_NAME'], 'Friendly Linux')
            self.assertEqual(info['BAD'], '$(touch do-not-create)')

    def test_arch_command_does_not_perform_partial_upgrade(self):
        self.assertEqual(online.PREFIXES['pacman'], ['pacman','-S','--needed','--noconfirm'])

    def test_missing_privilege_tool_has_clear_error(self):
        with patch.object(online.shutil,'which',return_value=None):
            with self.assertRaisesRegex(RuntimeError,'sudo or doas'):
                online.privileged(['true'])

    def test_doas_can_be_used(self):
        with patch.object(online.shutil,'which',side_effect=lambda name:name=='doas'), patch.object(online.subprocess,'run') as run:
            online.privileged(['apk','add','example'])
            run.assert_called_once_with(['doas','apk','add','example'],check=True)

    def test_no_systemd_does_not_invoke_systemctl(self):
        with patch.object(manage.shutil,'which',return_value=None), patch.object(manage.subprocess,'run') as run:
            self.assertFalse(manage.systemd_user_available())
            run.assert_not_called()

    def test_standalone_autostart_and_uninstall_preserve_settings(self):
        with tempfile.TemporaryDirectory() as temp, patch.dict(os.environ,{'XDG_CONFIG_HOME':temp+'/config','XDG_CACHE_HOME':temp+'/cache'}), patch.object(manage,'stop_standalone'), patch.object(manage.time,'sleep'), patch.object(manage.subprocess,'Popen') as popen:
            popen.return_value.poll.return_value=None
            manage.standalone_service('install', enable=True)
            entry=Path(temp)/'config/autostart/io.github.nirusaki.HuionDaemon.desktop'
            self.assertIn('Exec=/usr/local/bin/huion-daemon',entry.read_text())
            settings=Path(temp)/'config/huion-wayland/config.toml'
            settings.parent.mkdir(); settings.write_text('keep settings')
            manage.standalone_service('uninstall')
            self.assertFalse(entry.exists())
            self.assertEqual(settings.read_text(),'keep settings')

    def test_online_dry_run_has_no_install_or_network_calls(self):
        with patch.object(online.os,'geteuid',return_value=1000), patch.dict(os.environ,{'XDG_SESSION_TYPE':'wayland','DBUS_SESSION_BUS_ADDRESS':'test'}), patch.object(online,'os_release',return_value={'ID':'arch'}), patch.object(online,'rust_ready',return_value=True), patch.object(online,'privileged') as privileged, patch.object(online.subprocess,'run') as run, patch.object(online.urllib.request,'urlopen') as request, patch.object(online.sys,'argv',['installer','--dry-run']):
            online.main()
            privileged.assert_not_called(); run.assert_not_called(); request.assert_not_called()

    @unittest.skipIf(os.geteuid()==0, 'bootstrap intentionally rejects root')
    def test_curl_entrypoint_dry_run_download_and_cleanup(self):
        with tempfile.TemporaryDirectory() as temp:
            base=Path(temp)
            archive=base/'source.tar.gz'
            with tarfile.open(archive,'w:gz') as tar:
                tar.add(ROOT/'scripts/online_install.py',arcname='HuionHS64-Wayland-main/scripts/online_install.py')
            fake=base/'fake'; fake.mkdir()
            curl=fake/'curl'
            curl.write_text('#!/bin/sh\nwhile [ "$#" -gt 0 ]; do if [ "$1" = -o ]; then shift; cp "$KALAM_TEST_ARCHIVE" "$1"; printf "%s" "$1" > "$KALAM_TEST_DOWNLOAD_LOG"; exit 0; fi; shift; done; exit 1\n')
            curl.chmod(0o755)
            log=base/'download.log'
            env=dict(os.environ,PATH=str(fake)+os.pathsep+os.environ['PATH'],XDG_SESSION_TYPE='wayland',DBUS_SESSION_BUS_ADDRESS='test',KALAM_TEST_ARCHIVE=str(archive),KALAM_TEST_DOWNLOAD_LOG=str(log))
            result=subprocess.run(['sh',str(ROOT/'bootstrap.sh'),'--dry-run'],env=env,text=True,capture_output=True,check=True)
            self.assertIn('Desktop: Wayland', result.stdout)
            self.assertIn('Then: compile Kalam',result.stdout)
            self.assertFalse(Path(log.read_text()).parent.exists())
            log.unlink()
            env['XDG_SESSION_TYPE']='x11'
            result=subprocess.run(['sh',str(ROOT/'bootstrap.sh'),'--dry-run'],env=env,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0)
            self.assertFalse(log.exists())

    def test_x11_rejected_before_any_system_changes(self):
        with patch.object(online.os,'geteuid',return_value=1000), patch.dict(os.environ,{'XDG_SESSION_TYPE':'x11'},clear=True), patch.object(online,'privileged') as privileged, patch.object(online.sys,'argv',['installer']):
            with self.assertRaises(SystemExit): online.main()
            privileged.assert_not_called()


if __name__=='__main__':
    unittest.main()
