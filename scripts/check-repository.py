#!/usr/bin/env python3
"""Check public source hygiene and local Markdown links without printing secrets."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit


SECRET_PATTERNS = {
    "private key": re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    "provider token": re.compile(
        r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}"
        r"|AKIA[0-9A-Z]{16}|sk_live_[A-Za-z0-9]{20,})\b"
    ),
}
LINK = re.compile(r"!?\[[^\]\n]*\]\(([^)\n]+)\)")
TRANSLATION = re.compile(
    r"^<!-- translation-source: ([^\s;]+); source-sha256: ([0-9a-f]{64}) -->\r?$",
    re.MULTILINE,
)
PRIVATE_DIRS = {"output", "target", "dist", "node_modules", "private", ".playwright-cli"}
REQUIRED = (
    "README.md", "LICENSE", "NOTICE.md", "CONTRIBUTING.md", "SECURITY.md",
    "CODE_OF_CONDUCT.md", "SUPPORT.md", "docs/README.md", "docs/I18N.md",
    "docs/DEVELOPMENT.md", "docs/TESTING.md", "docs/RELEASE.md", "docs/MAINTAINING.md",
)


def check(root):
    errors = []
    for name in REQUIRED:
        if not (root / name).is_file():
            errors.append(f"{name}: missing required project document")
    license_path = root / "LICENSE"
    if license_path.is_file():
        text = license_path.read_text(encoding="utf-8")
        if "Apache License" not in text or "Version 2.0, January 2004" not in text:
            errors.append("LICENSE: expected Apache License 2.0")
    paths = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=root,
    ).decode("utf-8").split("\0")
    checked = 0
    for name in sorted(set(paths) - {""}):
        path = root / name
        # Gitlinks are directories, not vendored copies. Deleted paths are absent
        # while a contributor prepares a change; the staged tree is checked in CI.
        if not path.is_file():
            continue
        relative = Path(name)
        if (
            PRIVATE_DIRS.intersection(relative.parts)
            or (path.name.startswith(".env") and path.name != ".env.example")
            or re.search(r"\.(?:db(?:-.*)?|sqlite3?(?:-.*)?|pem|key|log|access\.txt)$", name)
        ):
            errors.append(f"{name}: runtime or private artifact must not be tracked")
        content = path.read_bytes()
        if b"\0" in content:
            continue
        try:
            text = content.decode("utf-8")
        except UnicodeDecodeError:
            continue
        checked += 1
        for label, pattern in SECRET_PATTERNS.items():
            for match in pattern.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                errors.append(f"{name}:{line}: possible {label}; value omitted")
        if path.suffix != ".md":
            continue
        if name == "README-zh.md" or relative.parts[:2] == ("docs", "zh-CN"):
            marker = TRANSLATION.search(text)
            if marker is None:
                errors.append(f"{name}: missing translation source revision")
            else:
                source = (root / marker.group(1)).resolve()
                if not source.is_relative_to(root) or not source.is_file():
                    errors.append(f"{name}: translation source is missing or outside the repository")
                elif hashlib.sha256(source.read_bytes()).hexdigest() != marker.group(2):
                    errors.append(f"{name}: English source changed; review and update the translation")
        for match in LINK.finditer(text):
            # Optional titles follow the target; this repository uses simple links.
            target = match.group(1).split(' "', 1)[0].strip("<>")
            url = urlsplit(target)
            if url.scheme or url.netloc or not url.path:
                continue
            destination = (path.parent / unquote(url.path)).resolve()
            if not destination.is_relative_to(root) or not destination.exists():
                line = text.count("\n", 0, match.start()) + 1
                errors.append(f"{name}:{line}: broken or out-of-tree local link: {target}")
    return checked, errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    checked, errors = check(args.root.resolve())
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Repository checks passed ({checked} text files).")
    print("This heuristic check does not replace history review or dedicated secret scanning.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
