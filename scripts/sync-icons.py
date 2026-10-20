#!/usr/bin/env python3
"""Generate desktop and browser icons from docs/assets/logo.png using Tauri."""
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    cli = ROOT / "apps/desktop/node_modules/@tauri-apps/cli/tauri.js"
    node = shutil.which("node")
    if not node or not cli.is_file():
        raise SystemExit("Install Node 24 and run npm ci --prefix apps/desktop first")
    with tempfile.TemporaryDirectory(prefix="holepunch-icons-") as directory:
        output = Path(directory)
        subprocess.run(
            [node, str(cli), "icon", str(ROOT / "docs/assets/logo.png"), "-o", directory],
            check=True,
        )
        for name in ["icon.png", "icon.ico", "icon.icns"]:
            shutil.copyfile(output / name, ROOT / "apps/desktop/src-tauri/icons" / name)
        shutil.copyfile(output / "128x128.png", ROOT / "web/logo.png")
        shutil.copyfile(output / "icon.ico", ROOT / "web/favicon.ico")
    print("Desktop and browser icons generated")


if __name__ == "__main__":
    main()
