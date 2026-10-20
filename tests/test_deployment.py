"""Portable installer regression tests; no host services or real credentials are changed."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path, PurePosixPath
import socket
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import Mock, patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('deployment', ROOT / 'scripts/deploy.py')
deploy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deploy)


class DeploymentTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def args(self, *extra):
        return deploy.parser().parse_args(['install', '--non-interactive', '--hide-secrets', '--public-host', 'edge.example.com',
                                          '--admin-email', 'admin@example.com', '--prefix', str(self.root / 'install'), *extra])

    def archive(self, edition='selfhost', target=None):
        bundle = self.root / 'release.tar.gz'
        with tarfile.open(bundle, 'w:gz') as archive:
            contents = {'release/BUILD.json': json.dumps({'edition': edition, 'target': target or deploy.target()}).encode()}
            ext = '.exe' if os.name == 'nt' else ''
            contents.update({f'release/bin/{name}{ext}': b'fixture' for name in ['holepunch-center', 'holepunch-client', 'rathole']})
            for name, value in contents.items():
                info = tarfile.TarInfo(name)
                info.size = len(value)
                archive.addfile(info, io.BytesIO(value))
        checksum = bundle.with_name(bundle.name + '.sha256')
        checksum.write_text(hashlib.sha256(bundle.read_bytes()).hexdigest() + '  ' + bundle.name + '\n')
        return bundle

    def test_checksum_rejects_tamper_wrong_name_and_duplicates(self):
        archive = self.archive()
        checksum = archive.with_name(archive.name + '.sha256')
        original = checksum.read_text()
        deploy.verify(archive, checksum)
        for value in [original.replace(archive.name, 'other.tar.gz'), original + original, '0' * 64 + '  ' + archive.name]:
            checksum.write_text(value)
            with self.assertRaises(ValueError):
                deploy.verify(archive, checksum)
        checksum.write_text(original)
        archive.write_bytes(archive.read_bytes() + b'tampered')
        with self.assertRaises(ValueError):
            deploy.verify(archive, checksum)

    def test_caddy_uses_upstream_sha512_checksums(self):
        archive = self.root / 'caddy.tar.gz'
        archive.write_bytes(b'upstream fixture')
        sums = self.root / 'caddy_checksums.txt'
        sums.write_text(hashlib.sha512(archive.read_bytes()).hexdigest() + '  ' + archive.name)
        deploy.verify(archive, sums, 'sha512')
        with self.assertRaises(ValueError):
            deploy.verify(archive, sums)

    def test_tar_rejects_traversal_links_and_devices(self):
        for index, (name, kind) in enumerate([('../escape', tarfile.REGTYPE), ('/absolute', tarfile.REGTYPE),
                                            ('link', tarfile.SYMTYPE), ('device', tarfile.CHRTYPE),
                                            ('hardlink', tarfile.LNKTYPE), ('C:/escape', tarfile.REGTYPE)]):
            archive = self.root / f'{index}.tar'
            with tarfile.open(archive, 'w') as output:
                member = tarfile.TarInfo(name)
                member.type = kind
                member.linkname = '/tmp/escape'
                output.addfile(member)
            with self.assertRaises(ValueError):
                deploy.extract(archive, self.root / f'extract-{index}')
        self.assertFalse((self.root / 'escape').exists())

    def test_zip_rejects_traversal_and_symlinks(self):
        for index, name in enumerate(['../escape', '..\\escape', 'C:/escape', '/absolute', 'link']):
            archive = self.root / f'{index}.zip'
            with zipfile.ZipFile(archive, 'w') as output:
                member = zipfile.ZipInfo(name)
                if name == 'link':
                    member.external_attr = (stat.S_IFLNK | 0o777) << 16
                output.writestr(member, b'escape')
            with self.assertRaises(ValueError):
                deploy.extract(archive, self.root / f'zip-{index}')

    def test_zip_extracts_windows_release_files(self):
        archive = self.root / 'normal.zip'
        with zipfile.ZipFile(archive, 'w') as output:
            output.writestr('release/bin/server.exe', b'binary')
        deploy.extract(archive, self.root / 'extracted')
        self.assertEqual((self.root / 'extracted/release/bin/server.exe').read_bytes(), b'binary')

    def test_rejects_wrong_edition_and_platform(self):
        for edition, target in [('cloud', deploy.target()), ('selfhost', 'wrong-platform')]:
            archive = self.archive(edition, target)
            with tempfile.TemporaryDirectory(dir=self.root) as work:
                with self.assertRaises(ValueError):
                    deploy.native_binaries(self.args('--archive', str(archive)), Path(work))

    def test_custom_ports_are_consistent(self):
        args = self.args('--center-port', '13020', '--caddy-port', '12050', '--control-port', '12400',
                         '--http-port', '18090', '--https-port', '18490')
        env = deploy.settings(args, self.root)
        self.assertEqual(env['HP_TLS_ASK_URL'], 'http://127.0.0.1:13020/api/v1/tls/check')
        self.assertEqual(env['HP_CADDY_URL'], 'http://127.0.0.1:12050')
        self.assertEqual(env['HP_CADDY_LISTEN'], '127.0.0.1:12050')
        self.assertEqual(env['HP_PUBLIC_ADDR'], 'edge.example.com:12400')
        self.assertEqual(env['HP_RATHOLE_BIND'], '0.0.0.0:12400')
        self.assertEqual(env['HP_HTTPS_LISTEN'], ':18490')
        self.assertGreaterEqual(len(env['HP_ADMIN_TOKEN']), 32)
        self.assertGreaterEqual(len(env['HP_BOOTSTRAP_PASSWORD']), 12)

    def test_rejects_config_injection_and_overlapping_ports(self):
        for extra in [('--public-host', 'host\nHP_ADMIN_TOKEN=x'), ('--public-host', 'https://example.com'),
                      ('--panel-domain', 'example.com/a'), ('--admin-email', 'a@example.com\n'),
                      ('--http-port', '443'), ('--control-port', '20000'), ('--port-end', '19999'),
                      ('--port-start', '1'), ('--center-port', '65536')]:
            with self.subTest(extra=extra), self.assertRaises(ValueError):
                deploy.settings(self.args(*extra), self.root)

    def test_listener_conflict_is_detected_without_stopping_owner(self):
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            listener.listen()
            port = listener.getsockname()[1]
            env = deploy.settings(self.args(), self.root)
            for name in ['HP_BIND', 'HP_CADDY_LISTEN', 'HP_RATHOLE_BIND', 'HP_HTTPS_LISTEN', 'HP_HTTP_PORT', 'HP_PORT_START', 'HP_PORT_END']:
                env[name] = str(port)
            with self.assertRaisesRegex(ValueError, 'existing services were not changed'):
                deploy.check_ports(env)
            self.assertEqual(listener.getsockname()[1], port)

    def test_fresh_install_preserves_secrets_on_repeat(self):
        archive = self.archive()
        caddy = self.root / 'caddy'
        caddy.write_bytes(b'fixture-caddy')
        args = self.args('--archive', str(archive), '--caddy', str(caddy))
        with patch.object(deploy, 'check_ports'):
            deploy.install(args)
        installed = args.prefix
        original = (installed / 'config.json').read_bytes()
        self.assertTrue((installed / 'deploy.py').exists())
        self.assertTrue((installed / 'state/caddy.json').exists())
        if os.name != 'nt':
            self.assertEqual(stat.S_IMODE((installed / 'config.json').stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(installed.stat().st_mode), 0o700)
        with self.assertRaisesRegex(ValueError, 'already exists'):
            deploy.install(args)
        self.assertEqual((installed / 'config.json').read_bytes(), original)

    def test_bad_download_leaves_no_install_directory(self):
        args = self.args('--version', '0.4.0')
        with patch.object(deploy, 'check_ports'), patch.object(deploy, 'download', side_effect=RuntimeError('unavailable')):
            with self.assertRaises(RuntimeError):
                deploy.install(args)
        self.assertFalse(args.prefix.exists())

    def test_supervisor_failure_reaps_children_and_filters_ambient_settings(self):
        prefix = self.root / 'runner'
        prefix.mkdir()
        (prefix / 'state/data').mkdir(parents=True)
        (prefix / 'state/data/server.toml').write_text('fixture')
        env = deploy.settings(self.args(), prefix)
        (prefix / 'config.json').write_text(json.dumps({'env': env}))
        children = [Mock(), Mock(), Mock()]
        for child in children:
            child.poll.return_value = None
        children[0].wait.side_effect = [subprocess.TimeoutExpired('caddy', 10), 0]
        with patch.object(deploy, 'check_ports'), patch.object(deploy.subprocess, 'Popen', side_effect=children) as launch, \
                patch.object(deploy, 'wait_ready', side_effect=RuntimeError('child failed')), \
                patch.dict(os.environ, {'HP_TLS_INTERNAL': '1', 'HP_BOOTSTRAP_PASSWORD': 'ambient-value'}):
            with self.assertRaisesRegex(RuntimeError, 'child failed'):
                deploy.run(prefix)
        for child in children:
            child.terminate.assert_called_once()
            self.assertTrue(child.wait.called)
        children[0].kill.assert_called_once()
        actual_env = launch.call_args_list[0].kwargs['env']
        self.assertNotIn('HP_TLS_INTERNAL', actual_env)
        self.assertEqual(actual_env['HP_BOOTSTRAP_PASSWORD'], env['HP_BOOTSTRAP_PASSWORD'])

    def test_udp_listener_conflict_is_detected(self):
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as listener:
            listener.bind(('0.0.0.0', 0))
            port = listener.getsockname()[1]
            env = deploy.settings(self.args(), self.root)
            for name in ['HP_BIND', 'HP_CADDY_LISTEN', 'HP_RATHOLE_BIND', 'HP_HTTPS_LISTEN', 'HP_HTTP_PORT', 'HP_PORT_START', 'HP_PORT_END']:
                env[name] = str(port)
            with self.assertRaisesRegex(ValueError, 'UDP port'):
                deploy.check_ports(env)

    def test_platform_mapping(self):
        for system, machine, expected in [('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'),
                                          ('Linux', 'aarch64', 'aarch64-unknown-linux-gnu'),
                                          ('Darwin', 'arm64', 'aarch64-apple-darwin'),
                                          ('Windows', 'AMD64', 'x86_64-pc-windows-msvc')]:
            with patch.object(deploy.platform, 'system', return_value=system), patch.object(deploy.platform, 'machine', return_value=machine):
                self.assertEqual(deploy.target(), expected)
        with patch.object(deploy.platform, 'system', return_value='FreeBSD'), self.assertRaises(ValueError):
            deploy.target()

    def test_systemd_unit_has_supervisor_and_scoped_writes(self):
        unit = deploy.unit_text(PurePosixPath('/opt/holepunch'), PurePosixPath('/usr/bin/python3'))
        self.assertIn('User=holepunch', unit)
        self.assertIn('ReadWritePaths=/opt/holepunch/state', unit)
        self.assertIn('KillMode=control-group', unit)
        self.assertIn('deploy.py run --prefix /opt/holepunch', unit)
        self.assertIn('AmbientCapabilities=CAP_NET_BIND_SERVICE', unit)
        with self.assertRaises(ValueError):
            deploy.unit_text(PurePosixPath('/opt/invalid%path'), PurePosixPath('/usr/bin/python3'))

    def test_compose_setup_is_exclusive_and_validates_custom_ports(self):
        destination = self.root / '.env'
        command = [sys.executable, str(ROOT / 'scripts/setup.py'), '--output', str(destination),
                   '--public-host', 'edge.example.com', '--center-port', '13020', '--caddy-port', '12050']
        subprocess.run(command, check=True, capture_output=True)
        original = destination.read_bytes()
        self.assertIn(b'HP_BOOTSTRAP_PASSWORD=', original)
        self.assertIn(b'HP_TLS_ASK_URL=http://127.0.0.1:13020/api/v1/tls/check', original)
        self.assertIn(b'HP_CADDY_PORT=12050', original)
        self.assertNotEqual(subprocess.run(command, capture_output=True).returncode, 0)
        self.assertEqual(destination.read_bytes(), original)


if __name__ == '__main__':
    unittest.main()
