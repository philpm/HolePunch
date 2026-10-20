#!/usr/bin/env python3
"""Generate platform icons and transparent logo variants using Tauri."""
import re
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
        assets = ROOT / "docs/assets"
        vector = (assets / "logo.svg").read_text()
        light, count = re.subn(r'(<svg\b[^>]*\bcolor=")[^"]+(")', r'\g<1>#FFFFFF\2', vector, count=1)
        if count != 1:
            raise SystemExit("docs/assets/logo.svg must specify a root color attribute")
        (assets / "logo-light.svg").write_text(light)
        for source, name in [("logo.svg", "logo-transparent.png"), ("logo-light.svg", "logo-light.png")]:
            destination = output / Path(source).stem
            subprocess.run(
                [node, str(cli), "icon", str(assets / source), "--png", "1024", "-o", str(destination)],
                check=True,
            )
            shutil.copyfile(destination / "1024x1024.png", assets / name)
    print("Platform icons and transparent logo variants generated")


if __name__ == "__main__":
    main()
