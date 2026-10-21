"""Release archive compatibility regression tests."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('packaging', ROOT / 'scripts/package.py')
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class PackagingTests(unittest.TestCase):
    def test_zip_retains_license_with_pre_1980_timestamp(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            stage = root / 'release'
            license_file = stage / 'licenses/example/LICENSE'
            license_file.parent.mkdir(parents=True)
            license_file.write_bytes(b'License text\n')
            os.utime(license_file, (1, 1))
            archive = root / 'release.zip'
            packaging.write_zip(stage, archive)
            with zipfile.ZipFile(archive) as bundle:
                name = 'release/licenses/example/LICENSE'
                self.assertEqual(bundle.read(name), b'License text\n')
                self.assertEqual(bundle.getinfo(name).date_time[:3], (1980, 1, 1))


if __name__ == '__main__':
    unittest.main()
