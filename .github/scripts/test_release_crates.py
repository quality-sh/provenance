"""Check release prerequisites without a network request or package upload."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ReleaseCratesTest(unittest.TestCase):
    def invoke(self, status):
        with tempfile.TemporaryDirectory() as directory:
            curl = Path(directory) / 'curl'
            curl.write_text('#!/bin/sh\nprintf "%s" "$TEST_STATUS"\n')
            curl.chmod(0o755)
            return subprocess.run(
                ['bash', str(ROOT / '.github/scripts/check-release-crates.sh')],
                env={**os.environ, 'PATH': directory + os.pathsep + os.environ['PATH'],
                     'TEST_STATUS': status}, capture_output=True, text=True, check=False)

    def test_all_existing_crates_pass(self):
        self.assertEqual(self.invoke('200').returncode, 0)

    def test_all_missing_crates_are_named(self):
        result = self.invoke('404')
        self.assertNotEqual(result.returncode, 0)
        for name in (ROOT / '.github/release-crates.txt').read_text().splitlines():
            self.assertIn(name, result.stderr)
        self.assertIn('publish a placeholder with an API token', result.stderr)
        self.assertIn('add the trusted publisher', result.stderr)

    def test_registry_error_stops_release(self):
        self.assertNotEqual(self.invoke('503').returncode, 0)


if __name__ == '__main__':
    unittest.main()
