"""Credential summaries and Compose readiness must agree with actual configuration."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from access import access_text, read_env, save_summary
spec = importlib.util.spec_from_file_location('compose_deployment', ROOT / 'scripts/compose.py')
compose = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compose)

ENV = {'HP_BIND':'127.0.0.1:13020', 'HP_BOOTSTRAP_EMAIL':'owner@example.test',
       'HP_BOOTSTRAP_PASSWORD':'test-initial-password', 'HP_ADMIN_TOKEN':'test-admin-token',
       'HP_PUBLIC_ADDR':'edge.example.test:2333', 'HP_PANEL_DOMAIN':'panel.example.test'}


class AccessTests(unittest.TestCase):
    def test_summary_lists_initial_credentials_and_explains_token(self):
        text = access_text(ENV, '/private/config.json')
        for value in ['https://panel.example.test','owner@example.test','test-initial-password','test-admin-token','Setup guide','not web sign-in or client enrollment']:
            self.assertIn(value, text)
        self.assertIn('HolePunch is ready.', text)
        hidden = access_text(ENV, '/private/config.json', hide_secrets=True)
        self.assertNotIn(ENV['HP_BOOTSTRAP_PASSWORD'], hidden)
        self.assertNotIn(ENV['HP_ADMIN_TOKEN'], hidden)
        self.assertIn(ENV['HP_BOOTSTRAP_EMAIL'], hidden)

    def test_no_domain_gives_ssh_instructions_and_prepared_is_not_ready(self):
        text = access_text({**ENV,'HP_PANEL_DOMAIN':''}, '/private/config.json', ready=False)
        self.assertIn('ssh -L 13020:127.0.0.1:13020 YOUR_SSH_USER@edge.example.test', text)
        self.assertIn('http://127.0.0.1:13020', text)
        self.assertNotIn('HolePunch is ready.', text)
        self.assertIn(':18443', access_text({**ENV,'HP_HTTPS_LISTEN':':18443'}, '/private/config.json'))

    def test_saved_summary_is_private_and_never_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'ACCESS.txt'
            save_summary(path, ENV, '/private/config.json')
            first = path.read_bytes()
            save_summary(path, {**ENV,'HP_ADMIN_TOKEN':'replacement'}, '/private/config.json')
            self.assertEqual(path.read_bytes(), first)
            if os.name != 'nt':
                self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

    def test_env_parser_never_executes_shell(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / '.env'
            path.write_text('# comment\nHP_ADMIN_TOKEN=$(touch should-not-exist)\nHP_BIND="127.0.0.1:3000"\n')
            values = read_env(path)
            self.assertEqual(values['HP_ADMIN_TOKEN'], '$(touch should-not-exist)')
            self.assertEqual(values['HP_BIND'], '127.0.0.1:3000')
            self.assertFalse((Path(temporary) / 'should-not-exist').exists())

    def test_compose_reports_ready_only_after_successful_probe(self):
        with patch.object(compose.subprocess,'run',return_value=Mock(returncode=0)) as run:
            compose.wait_ready(['docker','compose'])
            self.assertIn('/readyz', run.call_args.args[0][-1])
        with patch.object(compose.time,'monotonic',side_effect=[0,1,3]), patch.object(compose.time,'sleep'), patch.object(compose.subprocess,'run',return_value=Mock(returncode=1)):
            with self.assertRaises(RuntimeError):
                compose.wait_ready(['docker','compose'], seconds=2)

    def test_failed_compose_start_never_prints_success_or_credentials(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/'.env'
            path.write_text(''.join(f'{k}={v}\n' for k,v in ENV.items()))
            output=io.StringIO()
            with patch.object(sys,'argv',['compose.py','up','--env-file',str(path)]), patch.dict(os.environ), contextlib.redirect_stdout(output), contextlib.redirect_stderr(output), patch.object(compose.subprocess,'run',side_effect=OSError('Docker unavailable')):
                self.assertEqual(compose.main(),1)
            self.assertNotIn('HolePunch is ready.',output.getvalue())
            self.assertNotIn(ENV['HP_BOOTSTRAP_PASSWORD'],output.getvalue())


if __name__ == '__main__':
    unittest.main()
