"""The advisory service may be unavailable; that must never become a clean audit."""
import importlib.util
import io
from pathlib import Path
import unittest
import urllib.error
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    "dependency_audit", Path(__file__).resolve().parents[1] / "scripts/audit-dependencies.py"
)
AUDIT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT)


class DependencyAuditTests(unittest.TestCase):
    def test_transient_connection_failure_is_retried(self):
        with mock.patch.object(AUDIT.urllib.request, "urlopen", side_effect=[
            urllib.error.URLError("temporary connection failure"),
            io.BytesIO(b'{"results": []}'),
        ]) as request, mock.patch.object(AUDIT.time, "sleep"):
            self.assertEqual(AUDIT.request("https://api.osv.dev/v1/querybatch", {}), {"results": []})
            self.assertEqual(request.call_count, 2)

    def test_exhausted_retries_fail_closed(self):
        with mock.patch.object(AUDIT.urllib.request, "urlopen", side_effect=
            urllib.error.URLError("service unavailable")
        ) as request, mock.patch.object(AUDIT.time, "sleep"):
            with self.assertRaises(urllib.error.URLError):
                AUDIT.request("https://api.osv.dev/v1/querybatch", {})
            self.assertEqual(request.call_count, 3)

    def test_invalid_request_is_not_retried(self):
        error = urllib.error.HTTPError("https://api.osv.dev", 400, "bad request", {}, None)
        with mock.patch.object(AUDIT.urllib.request, "urlopen", side_effect=error) as request:
            with self.assertRaises(urllib.error.HTTPError):
                AUDIT.request("https://api.osv.dev/v1/querybatch", {})
            self.assertEqual(request.call_count, 1)


if __name__ == "__main__":
    unittest.main()
