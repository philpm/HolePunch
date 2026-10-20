#!/usr/bin/env python3
"""Generate protected Docker Compose settings without overwriting an existing file."""
import argparse
from pathlib import Path
import sys

from deploy import settings, write_private
from access import save_summary

SCRIPT_DIR = Path(__file__).resolve().parent
ROOT = SCRIPT_DIR.parent if SCRIPT_DIR.name == "scripts" else SCRIPT_DIR


def main(edition='selfhost'):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--interactive', action='store_true', help='Prompt for server, domain and administrator')
    parser.add_argument('--public-host')
    parser.add_argument('--panel-domain', default='')
    parser.add_argument('--edition', choices=[edition], default=edition)
    parser.add_argument('--admin-email', default='admin@example.test')
    parser.add_argument('--output', type=Path, default=ROOT / '.env')
    for name, default in [('center-port', 3000), ('caddy-port', 2019), ('control-port', 2333),
                          ('http-port', 80), ('https-port', 443), ('port-start', 20000), ('port-end', 20100)]:
        parser.add_argument('--' + name, type=int, default=default)
    args = parser.parse_args()
    try:
        if args.interactive:
            if not sys.stdin.isatty():
                raise ValueError('Interactive setup requires a terminal')
            args.public_host = args.public_host or input('Public hostname or IPv4: ').strip()
            args.panel_domain = args.panel_domain or input('Panel domain (blank for SSH access): ').strip()
            args.admin_email = input(f'Administrator email [{args.admin_email}]: ').strip() or args.admin_email
        if not args.public_host:
            raise ValueError('--public-host is required (or use --interactive)')
        values = settings(args, Path('/unused'))
        values = {key: value for key, value in values.items() if key.startswith('HP_') and key != 'HP_DATA_DIR'}
        values.update({'HP_EDITION': edition, 'HP_CADDY_PORT': str(args.caddy_port)})
        write_private(args.output, ''.join(f'{key}={value}\n' for key, value in values.items()))
    except (OSError, ValueError) as error:
        parser.error(str(error))
    save_summary(args.output.with_name(args.output.name + '.access.txt'), values, args.output.resolve())
    print(f'Created {args.output} with private credentials. Start and show browser access: python3 "{SCRIPT_DIR / "compose.py"}" up --env-file "{args.output}"')


if __name__ == '__main__':
    main()
