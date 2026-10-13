"""Tests gitleaks-scan.sh with the gitleaks binary in $GITLEAKS."""

import secrets
import string
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("gitleaks-scan.sh").resolve()


class GitleaksScanTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.repo = Path(temp.name)
        self.git("init", "-q", "-b", "work")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.test")
        token = "ghp_" + "".join(secrets.choice(string.ascii_letters + string.digits) for _ in range(36))
        (self.repo / "settings.txt").write_text(f'token = "{token}"\n')
        self.git("add", "settings.txt")
        self.git("commit", "-q", "-m", "leak")
        commit = self.git("rev-parse", "HEAD")
        self.fingerprint = f"{commit}:settings.txt:github-pat:1\n"

    def git(self, *args):
        result = subprocess.run(["git", *args], cwd=self.repo, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def scan(self):
        return subprocess.run(
            ["bash", str(SCRIPT), "--diff-merges=first-parent HEAD"],
            cwd=self.repo, capture_output=True, text=True,
        )

    def test_gitleaksignore_in_the_scanned_tree_does_not_hide_a_finding(self):
        (self.repo / ".gitleaksignore").write_text(self.fingerprint)
        self.git("add", ".gitleaksignore")
        self.git("commit", "-q", "-m", "ignore the leak")
        (self.repo / ".gitleaksignore").write_text(self.fingerprint)

        result = self.scan()

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("leaks found: 1", result.stdout + result.stderr)

    def test_gitleaksignore_on_origin_main_hides_the_finding(self):
        blob = subprocess.run(
            ["git", "hash-object", "-w", "--stdin"], cwd=self.repo, input=self.fingerprint,
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        tree = subprocess.run(
            ["git", "mktree"], cwd=self.repo, input=f"100644 blob {blob}\t.gitleaksignore\n",
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        main = self.git("commit-tree", tree, "-m", "main")
        self.git("update-ref", "refs/remotes/origin/main", main)

        result = self.scan()

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("no leaks found", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
