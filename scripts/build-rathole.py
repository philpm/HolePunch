#!/usr/bin/env python3
"""Build the pinned, patched rathole sidecar on Linux, macOS or Windows."""
import argparse
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from deploy import download, extract

ROOT = Path(__file__).resolve().parents[1]
SHA256 = '6358cf83f700408be4176f5bb96976404366154c384185cd79cfa8f43087ae4f'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('target', nargs='?')
    args = parser.parse_args()
    target = args.target or next(line[6:] for line in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
    root = ROOT / 'output/sidecar-source'
    root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=root) as temporary:
        work = Path(temporary)
        archive = work / 'rathole-0.5.0.crate'
        download('https://static.crates.io/crates/rathole/rathole-0.5.0.crate', archive)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
            raise ValueError('rathole source SHA-256 mismatch')
        extract(archive, work)
        fresh = work / 'rathole-0.5.0'
        with (fresh / 'Cargo.toml').open('a') as manifest:
            manifest.write('\n[workspace]\n')
        subprocess.run([sys.executable, str(ROOT / 'scripts/patch-rathole.py'), str(fresh)], check=True)
        shutil.copyfile(ROOT / 'deploy/rathole.Cargo.lock', fresh / 'Cargo.lock')
        source = root / 'rathole-0.5.0'
        # Preserve Cargo's build cache while refreshing the verified source files.
        shutil.copytree(fresh, source, dirs_exist_ok=True)
    subprocess.run(['cargo', 'build', '--manifest-path', str(source / 'Cargo.toml'), '--release', '--locked',
                    '--no-default-features', '--features', 'server,client,noise,hot-reload', '--target', target], check=True)
    extension = '.exe' if 'windows' in target else ''
    destination = ROOT / 'apps/desktop/src-tauri/binaries' / f'holepunch-rathole-{target}{extension}'
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source / 'target' / target / 'release' / ('rathole' + extension), destination)
    print(destination)


if __name__ == '__main__':
    main()
