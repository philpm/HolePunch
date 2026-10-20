#!/usr/bin/env python3
"""Exercise a real, isolated native installation without touching system services."""
import argparse
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
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
    parser.add_argument('--caddy', type=Path, required=True)
    parser.add_argument('--rathole', type=Path)
    args = parser.parse_args()
    if args.bin_dir and not args.rathole:
        parser.error('--rathole is required with --bin-dir')
    if os.name == 'nt':
        parser.error('This process-lifecycle smoke test requires POSIX signals; Windows runs the portable unit suite.')
    with tempfile.TemporaryDirectory(prefix='holepunch-native-smoke-') as temporary:
        root = Path(temporary)
        if args.archive:
            archive = args.archive.resolve()
        else:
            archive = root / 'native.tar.gz'
            with tarfile.open(archive, 'w:gz') as bundle:
                metadata = json.dumps({'edition': 'selfhost', 'target': deploy.target()}).encode()
                member = tarfile.TarInfo('release/BUILD.json')
                member.size = len(metadata)
                bundle.addfile(member, io.BytesIO(metadata))
                for path, name in [(args.bin_dir / 'holepunch-server', 'holepunch-center'),
                                   (args.bin_dir / 'holepunch-client', 'holepunch-client'),
                                   (args.rathole, 'rathole')]:
                    bundle.add(path.resolve(), arcname='release/bin/' + name)
            archive.with_name(archive.name + '.sha256').write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + '  ' + archive.name)
        probes = [socket.socket() for _ in range(6)]
        for probe in probes:
            probe.bind(('127.0.0.1', 0))
        ports = [probe.getsockname()[1] for probe in probes]
        for probe in probes:
            probe.close()
        prefix = root / 'installed'
        command = [sys.executable, str(ROOT / 'scripts/deploy.py'), 'install', '--non-interactive', '--hide-secrets',
                   '--archive', str(archive), '--caddy', str(args.caddy.resolve()), '--prefix', str(prefix),
                   '--public-host', '127.0.0.1', '--admin-email', 'native-smoke@example.test']
        for name, port in zip(['center-port', 'caddy-port', 'control-port', 'http-port', 'https-port', 'port-start'], ports):
            command.extend(['--' + name, str(port)])
        command.extend(['--port-end', str(ports[-1])])
        subprocess.run(command, check=True)
        config_path = prefix / 'config.json'
        config = json.loads(config_path.read_text())
        config['env'].update({'HP_TLS_INTERNAL': '1', 'HP_ALLOW_HTTP': '1'})
        config_path.write_text(json.dumps(config))
        original = config_path.read_bytes()
        for iteration in range(2):
            with (root / f'run-{iteration}.log').open('w+') as log:
                process = subprocess.Popen([sys.executable, str(prefix / 'deploy.py'), 'run', '--prefix', str(prefix)], stdout=log, stderr=subprocess.STDOUT)
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
                    print(log.read()[-6000:], file=sys.stderr)
                    raise
                finally:
                    process.send_signal(signal.SIGTERM)
                    try:
                        process.wait(timeout=35)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                        raise RuntimeError('Supervisor did not stop its children')
                if process.returncode:
                    log.seek(0)
                    raise RuntimeError('Native stack failed: ' + log.read()[-4000:])
            assert config_path.read_bytes() == original, 'Restart changed credentials'
            # Verify that no process retained the listeners after shutdown.
            for port in ports:
                with socket.socket() as probe:
                    assert probe.connect_ex(('127.0.0.1', port)) != 0, f'Port {port} leaked'
        print('Native smoke passed: verified archive, install, readiness, web login, restart, retained credentials and clean SIGTERM shutdown.')


if __name__ == '__main__':
    main()
