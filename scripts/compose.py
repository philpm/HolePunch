#!/usr/bin/env python3
"""Start Linux Compose, wait for real readiness, then show browser access and credentials."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import time

from access import access_text, read_env, save_summary

SCRIPT_DIR = Path(__file__).resolve().parent
ROOT = SCRIPT_DIR.parent if SCRIPT_DIR.name == "scripts" else SCRIPT_DIR
DEFAULT_COMPOSE = ROOT / ("docker-compose.yml" if (ROOT / "docker-compose.yml").exists() else "compose.release.yml")


def wait_ready(command, seconds=90):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        result = subprocess.run([*command, 'exec', '-T', 'center', 'sh', '-c',
                                 'curl --max-time 3 -fsS "http://${HP_BIND:-127.0.0.1:3000}/readyz"'],
                                capture_output=True, timeout=10)
        if result.returncode == 0:
            return
        time.sleep(1)
    raise RuntimeError('Compose started, but readiness timed out. Inspect compose logs; no success was reported.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['up'])
    parser.add_argument('--file', type=Path, default=DEFAULT_COMPOSE)
    parser.add_argument('--env-file', type=Path, default=ROOT / '.env')
    parser.add_argument('--pull', action='store_true', help='Pull published images instead of building source')
    parser.add_argument('--hide-secrets', action='store_true')
    args = parser.parse_args()
    try:
        if not args.env_file.is_file():
            raise ValueError('Generate configuration with python3 scripts/setup.py --interactive first.')
        env = read_env(args.env_file)
        command = ['docker', 'compose', '--env-file', str(args.env_file.resolve()), '-f', str(args.file.resolve())]
        # Avoid an ambient HP_* environment overriding the generated credentials/ports.
        for key, value in env.items():
            if key.startswith('HP_'):
                os.environ[key] = value
        if args.pull:
            subprocess.run([*command, 'pull'], check=True)
        subprocess.run([*command, 'up', '-d', '--wait', *([] if args.pull else ['--build'])], check=True)
        wait_ready(command)
        summary = args.env_file.with_name(args.env_file.name + '.access.txt')
        save_summary(summary, env, args.env_file.resolve())
        print(access_text(env, args.env_file.resolve(), args.hide_secrets), end='')
        print(f'Private initial-access summary: {summary.resolve()}')
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f'Error: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
