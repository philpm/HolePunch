#!/usr/bin/env python3
"""Exercise a real, isolated native installation without touching system services."""
import argparse
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('deployment', ROOT / 'scripts/deploy.py')
deploy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(deploy)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--bin-dir', type=Path)
    source.add_argument('--archive', type=Path)
    source.add_argument('--dist', type=Path)
    source.add_argument('--release-version', help='Verify the public download and version-based installer')
    parser.add_argument('--edition', choices=['selfhost', 'cloud'], default='selfhost')
    parser.add_argument('--expected-revision')
    parser.add_argument('--report', type=Path)
    parser.add_argument('--caddy', type=Path)
    parser.add_argument('--rathole', type=Path)
    args = parser.parse_args()
    if args.bin_dir and not args.rathole:
        parser.error('--rathole is required with --bin-dir')
    deploy.EDITION = args.edition
    deploy.REPOSITORY = 'philpm/HolePunch' + ('-Cloud' if args.edition == 'cloud' else '')
    ext = '.exe' if os.name == 'nt' else ''
    with tempfile.TemporaryDirectory(prefix='holepunch-native-smoke-') as temporary:
        root = Path(temporary)
        installer = ROOT / 'scripts/deploy.py'
        build = None
        if args.dist:
            archives = list(args.dist.glob(f'holepunch-{args.edition}-*-{deploy.target()}' + ('.zip' if ext else '.tar.gz')))
            if len(archives) != 1:
                raise ValueError('Expected exactly one native archive for this edition and target')
            archive = archives[0].resolve()
        elif args.release_version:
            version = args.release_version.removeprefix('v')
            if not re.fullmatch(r'\d+\.\d+\.\d+(?:[-.][A-Za-z0-9.-]+)?', version):
                parser.error('Invalid release version')
            name = f'holepunch-{args.edition}-{version}-{deploy.target()}' + ('.zip' if ext else '.tar.gz')
            archive = root / name
            base = f'https://github.com/{deploy.REPOSITORY}/releases/download/v{version}'
            for name in [archive.name, archive.name + '.sha256']:
                deploy.download(base + '/' + name, root / name)
        elif args.archive:
            archive = args.archive.resolve()
        else:
            archive = root / 'native.tar.gz'
            with tarfile.open(archive, 'w:gz') as bundle:
                metadata = json.dumps({'edition': args.edition, 'target': deploy.target()}).encode()
                member = tarfile.TarInfo('release/BUILD.json')
                member.size = len(metadata)
                bundle.addfile(member, io.BytesIO(metadata))
                server = 'holepunch-server' if args.edition == 'selfhost' else 'holepunch-cloud'
                for path, name in [(args.bin_dir / (server + ext), 'holepunch-center'),
                                   (args.bin_dir / ('holepunch-client' + ext), 'holepunch-client'),
                                   (args.rathole, 'rathole')]:
                    bundle.add(path.resolve(), arcname='release/bin/' + name + ext)
            archive.with_name(archive.name + '.sha256').write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + '  ' + archive.name)
        deploy.verify(archive, archive.with_name(archive.name + '.sha256'))
        if not args.bin_dir:
            deploy.extract(archive, root / 'package')
            builds = list((root / 'package').glob('*/BUILD.json'))
            if len(builds) != 1:
                raise ValueError('Expected one release root')
            package = builds[0].parent
            build = json.loads(builds[0].read_text(encoding='utf-8'))
            assert build['edition'] == args.edition and build['target'] == deploy.target()
            assert re.fullmatch(r'[0-9a-f]{40}', build['source_revision'])
            assert re.fullmatch(r'[0-9a-f]{40}', build['core_revision'])
            if args.expected_revision:
                assert build['source_revision'] == args.expected_revision, 'Source revision mismatch'
            if args.release_version:
                assert build['version'] == args.release_version.removeprefix('v')
            required = ['LICENSE', 'NOTICE.md', 'README.md', 'README-zh.md', 'docs/DEPLOYMENT.md',
                        'docs/zh-CN/DEPLOYMENT.md', 'DEPENDENCIES.json', 'deploy.py', 'access.py',
                        'install.sh', 'install.ps1', 'build-inputs/core.Cargo.lock', 'build-inputs/rathole.Cargo.lock']
            for name in required:
                assert (package / name).is_file(), f'Missing package file: {name}'
            assert 'Apache License' in (package / 'LICENSE').read_text(encoding='utf-8')
            inventories = json.loads((package / 'DEPENDENCIES.json').read_text(encoding='utf-8'))
            assert all(inventories[scope] for scope in ['core', 'client', 'rathole'])
            assert any((package / 'licenses').rglob('LICENSE*')), 'Missing dependency license texts'
            assert not any(path.name in {'.env', 'config.json', 'ACCESS.txt', '.git'} or path.suffix in {'.db', '.sqlite', '.pem', '.key'}
                           for path in package.rglob('*')), 'Runtime data in release archive'
            installer = package / 'deploy.py'
            for script in ['deploy.py', 'setup.py', 'compose.py', 'access.py']:
                subprocess.run([sys.executable, str(package / script), '--help'], cwd=root, check=True, capture_output=True)
        probes = [socket.socket() for _ in range(6)]
        for probe in probes:
            probe.bind(('127.0.0.1', 0))
        ports = [probe.getsockname()[1] for probe in probes]
        for probe in probes:
            probe.close()
        prefix = root / 'installed'
        command = [sys.executable, str(installer), 'install', '--non-interactive', '--hide-secrets',
                   '--prefix', str(prefix),
                   '--public-host', '127.0.0.1', '--admin-email', 'native-smoke@example.test']
        command.extend(['--version', args.release_version] if args.release_version else ['--archive', str(archive)])
        if args.caddy:
            command.extend(['--caddy', str(args.caddy.resolve())])
        for name, port in zip(['center-port', 'caddy-port', 'control-port', 'http-port', 'https-port', 'port-start'], ports):
            command.extend(['--' + name, str(port)])
        command.extend(['--port-end', str(ports[-1])])
        subprocess.run(command, cwd=root, check=True)
        if build:
            for binary in ['holepunch-center', 'holepunch-client']:
                output = subprocess.check_output([str(prefix / 'bin' / (binary + ext)), '--version'], text=True)
                assert build['version'] in output, f'{binary} version mismatch'
                if binary == 'holepunch-center':
                    edition = 'self_hosted' if args.edition == 'selfhost' else 'cloud'
                    assert '(' + edition + ')' in output, 'Server edition mismatch'
            subprocess.run([str(prefix / 'bin' / ('rathole' + ext)), '--version'], check=True)
        config_path = prefix / 'config.json'
        config = json.loads(config_path.read_text())
        config['env'].update({'HP_TLS_INTERNAL': '1', 'HP_ALLOW_HTTP': '1'})
        config_path.write_text(json.dumps(config))
        original = config_path.read_bytes()
        for iteration in range(2):
            with (root / f'run-{iteration}.log').open('w+') as log:
                process = subprocess.Popen([sys.executable, str(prefix / 'deploy.py'), 'run', '--prefix', str(prefix)],
                                           stdout=log, stderr=subprocess.STDOUT,
                                           creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0)
                try:
                    deploy.wait_ready(config['env'], [process], 60)
                    url = 'http://' + config['env']['HP_BIND']
                    with urllib.request.urlopen(url, timeout=5) as response:
                        assert response.status == 200
                    payload = json.dumps({'email': config['env']['HP_BOOTSTRAP_EMAIL'], 'password': config['env']['HP_BOOTSTRAP_PASSWORD']}).encode()
                    request = urllib.request.Request(url + '/api/v1/auth/login', data=payload, headers={'Content-Type': 'application/json'})
                    with urllib.request.urlopen(request, timeout=5) as response:
                        assert response.status == 200
                except Exception:
                    log.flush()
                    log.seek(0)
                    output = log.read()[-6000:]
                    for name in ['HP_ADMIN_TOKEN', 'HP_BOOTSTRAP_PASSWORD']:
                        output = output.replace(config['env'][name], '[redacted]')
                    print(output, file=sys.stderr)
                    raise
                finally:
                    if process.poll() is None:
                        process.send_signal(signal.CTRL_BREAK_EVENT if os.name == 'nt' else signal.SIGTERM)
                    try:
                        process.wait(timeout=35)
                    except subprocess.TimeoutExpired:
                        if os.name == 'nt':
                            subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], capture_output=True)
                        else:
                            process.kill()
                        process.wait()
                        raise RuntimeError('Supervisor did not stop its children')
                if process.returncode:
                    log.seek(0)
                    raise RuntimeError(f'Native stack exited with status {process.returncode}')
            assert config_path.read_bytes() == original, 'Restart changed credentials'
            # Verify that no process retained the listeners after shutdown.
            for port in ports:
                with socket.socket() as probe:
                    assert probe.connect_ex(('127.0.0.1', port)) != 0, f'Port {port} leaked'
        checks = ['sha256', 'fresh-install', 'readiness', 'web-login', 'restart', 'retained-credentials', 'clean-shutdown', 'no-leaked-listeners']
        if build:
            checks.extend(['metadata', 'binary-versions', 'edition', 'license-inventory', 'bilingual-docs', 'packaged-tools'])
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps({'archive': archive.name, 'sha256': hashlib.sha256(archive.read_bytes()).hexdigest(),
                                              'build': build, 'target': deploy.target(), 'checks': checks, 'result': 'passed'}, indent=2) + '\n', encoding='utf-8')
        print('Native smoke passed: ' + ', '.join(checks) + '.')


if __name__ == '__main__':
    main()
