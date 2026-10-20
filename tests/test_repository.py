"""Regression coverage for public repository hygiene without real credentials."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("repository_check", ROOT / "scripts/check-repository.py")
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class RepositoryTests(unittest.TestCase):
    def scan(self, root, files):
        with mock.patch.object(
            CHECK.subprocess, "check_output",
            return_value=("\0".join(files) + "\0").encode(),
        ):
            return CHECK.check(root)[1]

    def prepare(self, root):
        for name in CHECK.REQUIRED:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("# Document\n", encoding="utf-8")
        (root / "LICENSE").write_text("Apache License\nVersion 2.0, January 2004\n")

    def test_local_links_and_runtime_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            self.prepare(root)
            (root / "README.md").write_text(
                "[valid](docs/I18N.md#sources)\n[web](https://example.com)\n"
                "[missing](docs/missing.md)\n[escape](../outside.md)\n"
            )
            (root / ".env").write_text("EXAMPLE=value\n")
            (root / "state.db-wal").write_text("example\n")
            errors = self.scan(root, ["README.md", ".env", "state.db-wal"])
            self.assertEqual(len(errors), 4)
            self.assertTrue(any("missing.md" in error for error in errors))
            self.assertTrue(any("outside.md" in error for error in errors))
            self.assertTrue(any(".env:" in error for error in errors))
            self.assertTrue(any("state.db-wal:" in error for error in errors))

    def test_secret_findings_do_not_print_values(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            self.prepare(root)
            synthetic = "ghp_" + "x" * 36
            (root / "example.txt").write_text("example\n" + synthetic)
            errors = self.scan(root, ["example.txt"])
            self.assertEqual(len(errors), 1)
            self.assertIn("example.txt:2", errors[0])
            self.assertNotIn(synthetic, errors[0])

    def test_gitlink_and_untracked_runtime_files_are_not_scanned(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            self.prepare(root)
            (root / "vendor/core").mkdir(parents=True)
            (root / ".env").write_text("local-only\n")
            errors = self.scan(root, ["README.md", "vendor/core", "deleted.md"])
            self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
