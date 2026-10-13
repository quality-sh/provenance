"""Tests gitleaks-scan.sh with the gitleaks binary in $GITLEAKS.

Implementation aid with no Rule binding. Gitleaks always reads the
.gitleaksignore file in the scanned tree, so these tests pin that the script
ignores that copy and uses only the copy on origin/main. The tests also run
the Scan commits step of agent-cargo.yml. A dispatched run takes that step
from main, so the step must scan a snapshot that does not contain the script,
and a snapshot must not replace the script that the step runs.
"""

import os
import secrets
import string
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("gitleaks-scan.sh").resolve()
AGENT_CARGO = Path(__file__).resolve().parents[1] / "workflows" / "agent-cargo.yml"


def github_token():
    return "ghp_" + "".join(secrets.choice(string.ascii_letters + string.digits) for _ in range(36))


def scan_step():
    """Returns the run text of the Scan commits step of agent-cargo.yml."""
    lines = AGENT_CARGO.read_text().splitlines()
    start = lines.index("      - name: Scan commits")
    run = next(i for i in range(start, len(lines)) if lines[i].strip() == "run: |")
    body = []
    for line in lines[run + 1:]:
        if line.strip() and len(line) - len(line.lstrip()) <= 8:
            break
        body.append(line)
    return textwrap.dedent("\n".join(body)) + "\n"


class GitleaksScanTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.repo = Path(temp.name)
        self.git("init", "-q", "-b", "work")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.test")
        (self.repo / "settings.txt").write_text(f'token = "{github_token()}"\n')
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


class AgentCargoScanStepTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.repo = Path(temp.name) / "repo"
        self.runner_temp = Path(temp.name) / "runner"
        self.repo.mkdir()
        self.runner_temp.mkdir()
        self.git("init", "-q", "-b", "work")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.test")
        (self.repo / "README").write_text("before the scan script\n")
        self.git("add", "README")
        self.git("commit", "-q", "-m", "main before the scan script")
        self.fork = self.git("rev-parse", "HEAD")
        scripts = self.repo / ".github" / "scripts"
        scripts.mkdir(parents=True)
        (scripts / "gitleaks-scan.sh").write_text(SCRIPT.read_text())
        (self.repo / ".gitleaks.toml").write_text("[extend]\nuseDefault = true\n")
        self.git("add", ".")
        self.git("commit", "-q", "-m", "main with the scan script")
        self.main = self.git("rev-parse", "HEAD")
        self.git("update-ref", "refs/remotes/origin/main", self.main)
        self.git("checkout", "-q", "--detach", self.fork)

    def git(self, *args):
        result = subprocess.run(["git", *args], cwd=self.repo, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def snapshot(self, files):
        for name, text in files.items():
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        self.git("add", ".")
        self.git("commit", "-q", "-m", "snapshot")
        return self.git("rev-parse", "HEAD")

    def run_step(self, snapshot):
        env = {
            "PATH": os.environ["PATH"],
            "GITLEAKS": os.environ["GITLEAKS"],
            "GITHUB_SHA": self.main,
            "SNAPSHOT_SHA": snapshot,
            "RUNNER_TEMP": str(self.runner_temp),
        }
        return subprocess.run(
            ["bash", "-c", scan_step()], cwd=self.repo, env=env, capture_output=True, text=True,
        )

    def test_snapshot_without_the_scan_script_scans_clean(self):
        snapshot = self.snapshot({"notes.txt": "no secret here\n"})

        result = self.run_step(snapshot)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("no leaks found", result.stdout + result.stderr)

    def test_snapshot_without_the_scan_script_reports_its_leak(self):
        snapshot = self.snapshot({"settings.txt": f'token = "{github_token()}"\n'})

        result = self.run_step(snapshot)

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("leaks found: 1", result.stdout + result.stderr)

    def test_snapshot_cannot_replace_the_scan_script(self):
        snapshot = self.snapshot({
            ".github/scripts/gitleaks-scan.sh": "#!/usr/bin/env bash\nexit 0\n",
            "settings.txt": f'token = "{github_token()}"\n',
        })

        result = self.run_step(snapshot)

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("leaks found: 1", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
