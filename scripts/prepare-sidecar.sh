#!/usr/bin/env bash
# Compatibility entry point; the source build is shared with Windows/Python installs.
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 scripts/build-rathole.py "$@"
