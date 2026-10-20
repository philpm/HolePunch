#!/bin/sh
# Run a downloaded copy so interactive prompts retain the terminal's stdin.
set -eu
command -v python3 >/dev/null 2>&1 || { echo 'Python 3.10+ is required. Install python3 using your OS package manager.' >&2; exit 1; }
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -f "$script_dir/deploy.py" ]; then
  exec python3 "$script_dir/deploy.py" install "$@"
fi
: "${HOLEPUNCH_VERSION:?Set HOLEPUNCH_VERSION to an existing release tag, for example v0.4.0}"
case "$HOLEPUNCH_VERSION" in *[!a-zA-Z0-9.-]*) echo 'Invalid release tag' >&2; exit 1;; esac
command -v curl >/dev/null 2>&1 || { echo 'curl is required.' >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
curl --proto '=https' --tlsv1.2 -fsSL "https://raw.githubusercontent.com/philpm/HolePunch/$HOLEPUNCH_VERSION/scripts/deploy.py" -o "$work/deploy.py"
curl --proto '=https' --tlsv1.2 -fsSL "https://raw.githubusercontent.com/philpm/HolePunch/$HOLEPUNCH_VERSION/scripts/access.py" -o "$work/access.py"
python3 "$work/deploy.py" install --version "$HOLEPUNCH_VERSION" "$@"
