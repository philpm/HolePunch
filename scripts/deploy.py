#!/usr/bin/env python3
"""Install and supervise a native HolePunch stack using Python's standard library."""
import argparse
import hashlib
import ipaddress
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import secrets
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
import zipfile

CADDY_VERSION = '2.11.4'
REPOSITORY = 'philpm/HolePunch'
EDITION = 'selfhost'
from access import access_text, save_summary
TARGETS = {
    ('Linux', 'x86_64'): 'x86_64-unknown-linux-gnu',
    ('Linux', 'aarch64'): 'aarch64-unknown-linux-gnu',
    ('Darwin', 'x86_64'): 'x86_64-apple-darwin',
    ('Darwin', 'aarch64'): 'aarch64-apple-darwin',
    ('Windows', 'x86_64'): 'x86_64-pc-windows-msvc',
}


def target():
    machine = platform.machine().lower()
    machine = {'amd64': 'x86_64', 'arm64': 'aarch64'}.get(machine, machine)
    try:
        return TARGETS[(platform.system(), machine)]
    except KeyError:
        raise ValueError(f'Unsupported native platform: {platform.system()} {machine}. Build from source on a supported Rust target.') from None


def host(value):
    """Accept DNS names and IPv4; reject URL, shell and config syntax."""
    if len(value) > 253:
        raise ValueError('Hostname is too long')
    try:
        return str(ipaddress.IPv4Address(value))
    except ValueError:
        pass
    if not all(re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?', label)
               for label in value.split('.')):
        raise ValueError('Use an ASCII DNS hostname or IPv4 address, without scheme, path or port')
    return value.lower()


def email(value):
    if not re.fullmatch(r'[A-Za-z0-9.!+_%-]+@[A-Za-z0-9.-]+', value):
        raise ValueError('Use a valid administrator email address')
    host(value.split('@')[1])
    return value


def download(url, destination):
    print(f'Downloading {url}')
    request = urllib.request.Request(url, headers={'User-Agent': 'HolePunch-installer'})
    try:
        with urllib.request.urlopen(request, timeout=60) as response, destination.open('wb') as output:
            shutil.copyfileobj(response, output)
    except OSError as error:
        raise RuntimeError('Download failed. Check connectivity and that this release asset exists; '
                           'unpublished development builds require --source or --archive.') from error


def verify(archive, checksum_file, algorithm='sha256'):
    matches = []
    for line in checksum_file.read_text(encoding='utf-8').splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[1].lstrip('*') == archive.name:
            matches.append(fields[0].lower())
    hasher = hashlib.new(algorithm)
    with archive.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            hasher.update(block)
    digest = hasher.hexdigest()
    if matches != [digest]:
        raise ValueError(f'{algorithm.upper()} verification failed for {archive.name}')


def extract(archive, destination):
    """Extract regular files only; reject traversal, links and special devices."""
    def checked(name):
        path = PurePosixPath(name)
        if path.is_absolute() or '..' in path.parts or '\\' in name or ':' in name:
            raise ValueError('Unsafe archive member')
        result = destination.joinpath(*path.parts)
        if not result.resolve().is_relative_to(destination.resolve()):
            raise ValueError('Archive member escapes destination')
        return result

    if archive.suffix == '.zip':
        with zipfile.ZipFile(archive) as bundle:
            for member in bundle.infolist():
                path = checked(member.filename)
                mode = member.external_attr >> 16
                if stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR):
                    raise ValueError('Archive links and special files are not supported')
                if member.is_dir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with bundle.open(member) as source, path.open('xb') as output:
                        shutil.copyfileobj(source, output)
    else:
        with tarfile.open(archive) as bundle:
            for member in bundle:
                path = checked(member.name)
                if member.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                elif member.isfile():
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with bundle.extractfile(member) as source, path.open('xb') as output:
                        shutil.copyfileobj(source, output)
                else:
                    raise ValueError('Archive links and special files are not supported')


def private_directory(path):
    path.mkdir(mode=0o700, parents=True, exist_ok=False)
    if os.name == 'nt':
        identity = subprocess.check_output(['whoami'], text=True).strip()
        subprocess.run(['icacls', str(path), '/inheritance:r', '/grant:r',
                        f'{identity}:(OI)(CI)F', '*S-1-5-18:(OI)(CI)F'], check=True,
                       stdout=subprocess.DEVNULL)
    else:
        path.chmod(0o700)


def write_private(path, value):
    fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, 'w', encoding='utf-8') as output:
        output.write(value)


def settings(args, prefix):
    values = [args.center_port, args.caddy_port, args.control_port, args.http_port, args.https_port]
    if any(p < 1 or p > 65535 for p in values) or len(set(values)) != len(values):
        raise ValueError('Listener ports must be distinct and within 1..65535')
    if not 1024 <= args.port_start <= args.port_end <= 65535:
        raise ValueError('Tunnel range must be ordered and within 1024..65535')
    if any(args.port_start <= p <= args.port_end for p in values):
        raise ValueError('Tunnel range overlaps a service listener')
    public = host(args.public_host)
    panel = host(args.panel_domain) if args.panel_domain else ''
    if panel and ('.' not in panel or re.fullmatch(r'[0-9.]+', panel)):
        raise ValueError('Panel domain must be a fully qualified DNS name')
    return {
        'HP_ADMIN_TOKEN': secrets.token_hex(32),
        'HP_BOOTSTRAP_EMAIL': email(args.admin_email),
        'HP_BOOTSTRAP_PASSWORD': secrets.token_urlsafe(24),
        'HP_DATA_DIR': str(prefix / 'state' / 'data'),
        'HP_BIND': f'127.0.0.1:{args.center_port}',
        'HP_PUBLIC_ADDR': f'{public}:{args.control_port}',
        'HP_RATHOLE_BIND': f'0.0.0.0:{args.control_port}',
        'HP_PANEL_DOMAIN': panel,
        'HP_CADDY_LISTEN': f'127.0.0.1:{args.caddy_port}',
        'HP_CADDY_URL': f'http://127.0.0.1:{args.caddy_port}',
        'HP_TLS_ASK_URL': f'http://127.0.0.1:{args.center_port}/api/v1/tls/check',
        'HP_HTTP_PORT': str(args.http_port),
        'HP_HTTPS_LISTEN': f':{args.https_port}',
        'HP_PORT_START': str(args.port_start), 'HP_PORT_END': str(args.port_end),
        'XDG_DATA_HOME': str(prefix / 'state' / 'caddy-data'),
        'XDG_CONFIG_HOME': str(prefix / 'state' / 'caddy-config'),
        'RUST_LOG': 'info',
    }


def check_ports(env):
    ports = {int(env[name].rsplit(':', 1)[-1]) for name in
             ['HP_BIND', 'HP_CADDY_LISTEN', 'HP_RATHOLE_BIND', 'HP_HTTPS_LISTEN', 'HP_HTTP_PORT']}
    ports.update(range(int(env['HP_PORT_START']), int(env['HP_PORT_END']) + 1))
    for port in sorted(ports):
        # BSD permits a reused wildcard bind beside a specific-address listener.
        # Check loopback explicitly before allowing TIME_WAIT reuse below.
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as active:
            active.settimeout(0.1)
            if active.connect_ex(('127.0.0.1', port)) == 0:
                raise ValueError(f'TCP port {port} is occupied; existing services were not changed.')
        for kind in (socket.SOCK_STREAM, socket.SOCK_DGRAM):
            with socket.socket(socket.AF_INET, kind) as probe:
                if os.name == 'nt':
                    probe.setsockopt(socket.SOL_SOCKET, socket.SO_EXCLUSIVEADDRUSE, 1)
                elif kind == socket.SOCK_STREAM:
                    # Allow TIME_WAIT after a clean restart; listen still rejects a live owner.
                    probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                try:
                    probe.bind(('0.0.0.0', port))
                    if kind == socket.SOCK_STREAM:
                        probe.listen(1)
                except OSError as error:
                    protocol = 'TCP' if kind == socket.SOCK_STREAM else 'UDP'
                    raise ValueError(f'Cannot bind {protocol} port {port}: {error}. '
                                     'Choose unused ports; existing services were not changed. See docs/COEXISTENCE.md.') from error


def caddy_binary(work, supplied):
    if supplied:
        return supplied.resolve()
    system = {'Linux': 'linux', 'Darwin': 'mac', 'Windows': 'windows'}[platform.system()]
    arch = 'amd64' if target().startswith('x86_64') else 'arm64'
    extension = '.zip' if os.name == 'nt' else '.tar.gz'
    name = f'caddy_{CADDY_VERSION}_{system}_{arch}{extension}'
    base = f'https://github.com/caddyserver/caddy/releases/download/v{CADDY_VERSION}'
    archive, sums = work / name, work / 'caddy-checksums.txt'
    download(f'{base}/{name}', archive)
    download(f'{base}/caddy_{CADDY_VERSION}_checksums.txt', sums)
    verify(archive, sums, 'sha512')
    extract(archive, work / 'caddy')
    return work / 'caddy' / ('caddy.exe' if os.name == 'nt' else 'caddy')


def native_binaries(args, work):
    ext = '.exe' if os.name == 'nt' else ''
    if args.source:
        source = args.source.resolve()
        subprocess.run(['cargo', 'build', '--release', '--locked', '-p', 'holepunch-server', '-p', 'holepunch-cli'], cwd=source, check=True)
        subprocess.run([sys.executable, str(source / 'scripts/build-rathole.py')], cwd=source, check=True)
        return {'holepunch-center': source / 'target/release' / ('holepunch-server' + ext),
                'holepunch-client': source / 'target/release' / ('holepunch-client' + ext),
                'rathole': source / 'apps/desktop/src-tauri/binaries' / f'holepunch-rathole-{target()}{ext}'}
    archive = args.archive
    if archive:
        archive = archive.resolve()
        sums = args.checksum or archive.with_name(archive.name + '.sha256')
    else:
        version = args.version
        if not version or not re.fullmatch(r'v?\d+\.\d+\.\d+(?:[-.][A-Za-z0-9.-]+)?', version):
            raise ValueError('Specify --version for a published release, --archive for a local archive, or --source for a checkout')
        version = version.removeprefix('v')
        name = f'holepunch-{EDITION}-{version}-{target()}' + ('.zip' if ext else '.tar.gz')
        archive, sums = work / name, work / (name + '.sha256')
        base = f'https://github.com/{REPOSITORY}/releases/download/v{version}'
        download(f'{base}/{name}', archive)
        download(f'{base}/{name}.sha256', sums)
    verify(archive, sums)
    extract(archive, work / 'release')
    builds = list((work / 'release').glob('*/BUILD.json'))
    if len(builds) != 1:
        raise ValueError('Expected one release root with BUILD.json')
    build = json.loads(builds[0].read_text())
    if args.version and build.get('version') != args.version.removeprefix('v'):
        raise ValueError('Release version does not match the requested version')
    if build.get('edition') != EDITION or build.get('target') != target():
        raise ValueError('Release edition or platform does not match this installation')
    return {name: builds[0].parent / 'bin' / (name + ext)
            for name in ['holepunch-center', 'holepunch-client', 'rathole']}


def unit_text(prefix, python):
    # Restrict paths before embedding them in systemd's separate quoting language.
    if not all(re.fullmatch(r'/[A-Za-z0-9_./-]+', str(p)) for p in [prefix, python]):
        raise ValueError('systemd installation and Python paths cannot contain spaces or special characters')
    return f'''[Unit]
Description=HolePunch stack
Wants=network-online.target
After=network-online.target
[Service]
Type=simple
User=holepunch
Group=holepunch
ExecStart={python} {prefix}/deploy.py run --prefix {prefix}
Restart=on-failure
RestartSec=5
KillMode=control-group
TimeoutStopSec=45
UMask=0077
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths={prefix}/state
AmbientCapabilities=CAP_NET_BIND_SERVICE
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
[Install]
WantedBy=multi-user.target
'''


def install_service(prefix, unit):
    import grp
    import pwd
    try:
        pwd.getpwnam('holepunch')
    except KeyError:
        subprocess.run(['useradd', '--system', '--user-group', '--no-create-home',
                        '--shell', '/usr/sbin/nologin', 'holepunch'], check=True)
    user, group = pwd.getpwnam('holepunch'), grp.getgrnam('holepunch')
    os.chown(prefix, 0, group.gr_gid)
    prefix.chmod(0o750)
    os.chown(prefix / 'config.json', 0, group.gr_gid)
    (prefix / 'config.json').chmod(0o640)
    for path in [prefix / 'state', *(prefix / 'state').rglob('*')]:
        os.chown(path, user.pw_uid, group.gr_gid)
    write_private(Path('/etc/systemd/system/holepunch.service'), unit)
    Path('/etc/systemd/system/holepunch.service').chmod(0o644)
    subprocess.run(['systemctl', 'daemon-reload'], check=True)
    subprocess.run(['systemctl', 'enable', '--now', 'holepunch.service'], check=True)
    wait_ready(json.loads((prefix / 'config.json').read_text())['env'], [], 60)


def install(args):
    if not args.non_interactive:
        if not sys.stdin.isatty():
            raise ValueError('Interactive installation needs a terminal. Download the script first, or use --non-interactive with explicit options.')
        args.public_host = args.public_host or input('Public server hostname or IPv4: ').strip()
        args.panel_domain = args.panel_domain or input('Panel domain (blank for SSH access): ').strip()
        args.admin_email = args.admin_email or input('Administrator email: ').strip()
        if not (args.source or args.archive or args.version):
            args.version = input('Published release version (for example 0.4.0): ').strip()
        for attribute, title in [('center_port', 'Local panel port'), ('caddy_port', 'Local Caddy admin port'),
                                 ('control_port', 'Public control port'), ('http_port', 'HTTP port'),
                                 ('https_port', 'HTTPS port'), ('port_start', 'First public tunnel port'),
                                 ('port_end', 'Last public tunnel port')]:
            value = input(f'{title} [{getattr(args, attribute)}]: ').strip()
            if value:
                setattr(args, attribute, int(value))
    if not args.public_host or not args.admin_email:
        raise ValueError('--public-host and --admin-email are required')
    prefix = args.prefix.expanduser().absolute()
    if prefix.exists():
        raise ValueError(f'{prefix} already exists. Installation never overwrites data or credentials; see the upgrade guide.')
    unit = None
    if args.service == 'systemd':
        if platform.system() != 'Linux' or os.geteuid() != 0 or not Path('/run/systemd/system').is_dir():
            raise ValueError('systemd installation requires root on a Linux host running systemd')
        if Path('/etc/systemd/system/holepunch.service').exists():
            raise ValueError('holepunch.service already exists; use the upgrade guide')
        if str(prefix).startswith(('/home/', '/root/')):
            raise ValueError('Use a system path such as /opt/holepunch for systemd')
        unit = unit_text(prefix, Path(sys.executable).resolve())
    env = settings(args, prefix)
    check_ports(env)
    print(f'Install directory: {prefix}\nPlatform: {target()}\nRun mode: {args.service}')
    print('Firewall rules and DNS are not modified. Public HTTPS needs port 80/443 forwarding to the configured HTTP/HTTPS ports.')
    if not args.non_interactive and input('Install this configuration? [y/N]: ').strip().lower() != 'y':
        print('Cancelled; no installation was created.')
        return
    # Download and validate everything before creating the installation directory.
    with tempfile.TemporaryDirectory(prefix='holepunch-install-') as temporary:
        work = Path(temporary)
        binaries = native_binaries(args, work)
        binaries['caddy'] = caddy_binary(work, args.caddy)
        for name, binary in binaries.items():
            if not binary.is_file():
                raise ValueError(f'Missing binary: {name}')
        private_directory(prefix)
        (prefix / 'bin').mkdir()
        (prefix / 'state').mkdir(mode=0o700)
        ext = '.exe' if os.name == 'nt' else ''
        for name, binary in binaries.items():
            destination = prefix / 'bin' / (name + ext)
            shutil.copyfile(binary, destination)
            destination.chmod(0o755)
        shutil.copyfile(__file__, prefix / 'deploy.py')
        shutil.copyfile(Path(__file__).with_name('access.py'), prefix / 'access.py')
        write_private(prefix / 'config.json', json.dumps({'env': env}, indent=2) + '\n')
        write_private(prefix / 'state/caddy.json', json.dumps({'admin': {'listen': env['HP_CADDY_LISTEN']}, 'apps': {}}))
    save_summary(prefix / 'ACCESS.txt', env, prefix / 'config.json')
    if unit:
        install_service(prefix, unit)
        print('Installed and ready. Logs: journalctl -u holepunch -f')
    else:
        print(f'Installed. Start: python3 "{prefix / "deploy.py"}" run --prefix "{prefix}"')
    print(access_text(env, prefix / 'config.json', args.hide_secrets, ready=bool(unit)), end='')
    print(f'Private initial-access summary: {prefix / "ACCESS.txt"}')


def wait_ready(env, processes, seconds):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if any(process.poll() is not None for process in processes):
            raise RuntimeError('A stack process exited; inspect its preceding log output')
        try:
            with urllib.request.urlopen(f'http://{env["HP_BIND"]}/readyz', timeout=1) as response:
                if response.status == 200:
                    return
        except OSError:
            pass
        time.sleep(0.25)
    raise RuntimeError('Readiness timed out; inspect logs, DNS and listener configuration')


def run(prefix):
    prefix = prefix.expanduser().resolve()
    config = json.loads((prefix / 'config.json').read_text(encoding='utf-8'))
    # Prevent a caller's development/Cloud HP_* variables from changing this instance.
    env = {key: value for key, value in os.environ.items() if not key.startswith('HP_')}
    env.update(config['env'])
    check_ports(env)
    processes = []
    ext = '.exe' if os.name == 'nt' else ''
    def launch(name, *arguments):
        process = subprocess.Popen([str(prefix / 'bin' / (name + ext)), *map(str, arguments)],
                                   cwd=prefix, env=env, stdin=subprocess.DEVNULL)
        processes.append(process)
        return process
    def stop_requested(signum, frame):
        raise KeyboardInterrupt
    signals = [signal.SIGINT, signal.SIGTERM]
    if os.name == 'nt':
        signals.append(signal.SIGBREAK)
    previous = {s: signal.signal(s, stop_requested) for s in signals}
    try:
        launch('caddy', 'run', '--config', prefix / 'state/caddy.json')
        center = launch('holepunch-center')
        config_path = Path(env['HP_DATA_DIR']) / 'server.toml'
        for _ in range(200):
            if center.poll() is not None:
                raise RuntimeError('Center exited before generating rathole configuration')
            if config_path.is_file():
                break
            time.sleep(0.1)
        else:
            raise RuntimeError('Center did not generate rathole configuration')
        launch('rathole', '--server', config_path)
        wait_ready(env, processes, 60)
        if sys.stdout.isatty():
            print(access_text(env, prefix / 'config.json', hide_secrets=True), end='')
        print(f'HolePunch is ready at http://{env["HP_BIND"]}. Press Ctrl+C to stop.', flush=True)
        while True:
            if any(process.poll() is not None for process in processes):
                raise RuntimeError('A stack process exited; stopping the remaining processes')
            time.sleep(0.5)
    except KeyboardInterrupt:
        print('Stopping HolePunch; data and credentials are retained.', flush=True)
    finally:
        # Ignore repeated interrupts until every owned child has been reaped.
        for sig in previous:
            signal.signal(sig, signal.SIG_IGN)
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest='action', required=True)
    install_parser = commands.add_parser('install', help='Interactive fresh installation; existing directories are never overwritten')
    sources = install_parser.add_mutually_exclusive_group()
    sources.add_argument('--source', type=Path, help='Build from this source checkout (Rust and build tools required)')
    sources.add_argument('--archive', type=Path, help='Install a locally built release archive with its .sha256 file')
    sources.add_argument('--version', help='Download an existing published GitHub release version')
    install_parser.add_argument('--checksum', type=Path, help='Checksum file for --archive')
    install_parser.add_argument('--caddy', type=Path, help=f'Use an existing Caddy {CADDY_VERSION} binary instead of downloading it')
    install_parser.add_argument('--prefix', type=Path, default=Path.home() / '.local/share/holepunch')
    install_parser.add_argument('--service', choices=['foreground', 'systemd'], default='foreground')
    install_parser.add_argument('--non-interactive', action='store_true')
    install_parser.add_argument('--hide-secrets', action='store_true', help='Hide initial password/token in terminal output')
    install_parser.add_argument('--public-host')
    install_parser.add_argument('--panel-domain', default='')
    install_parser.add_argument('--admin-email')
    for name, default in [('center-port', 3000), ('caddy-port', 2019), ('control-port', 2333),
                          ('http-port', 80), ('https-port', 443), ('port-start', 20000), ('port-end', 20100)]:
        install_parser.add_argument('--' + name, type=int, default=default)
    for action in ['run', 'check']:
        command = commands.add_parser(action, help='Run all three processes' if action == 'run' else 'Check readiness of an installed stack')
        command.add_argument('--prefix', type=Path, required=True)
    return result


def main():
    if sys.version_info < (3, 10):
        print('Python 3.10+ is required.', file=sys.stderr)
        return 1
    args = parser().parse_args()
    try:
        if args.action == 'install':
            install(args)
        elif args.action == 'run':
            run(args.prefix)
        else:
            env = json.loads((args.prefix.expanduser() / 'config.json').read_text())['env']
            wait_ready(env, [], 3)
            print('HolePunch is ready.')
    except (ValueError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f'Error: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
