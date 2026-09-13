import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


WRAPPER = Path(__file__).resolve().parents[2] / "scripts" / "agent-bin" / "cargo"


def run(*args, cwd, env=None):
    return subprocess.run(args, cwd=cwd, env=env, text=True, capture_output=True, check=False)


class AgentCargoTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "provenance"
        self.repo.mkdir()
        self.remote = self.root / "remote.git"
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.calls = self.root / "gh-calls.jsonl"
        self.payload = self.root / "gh-payload.json"

        self.git("init", "-b", "main")
        self.git("config", "user.name", "Test Agent")
        self.git("config", "user.email", "agent@example.test")
        (self.repo / "Cargo.toml").write_text(
            '[workspace]\nmembers = []\n[workspace.package]\nrepository = "https://github.com/quality-sh/provenance"\n'
        )
        (self.repo / "src.rs").write_text("before\n")
        self.git("add", ".")
        self.git("commit", "-m", "base")
        bare = run("git", "init", "--bare", str(self.remote), cwd=self.root)
        self.assertEqual(bare.returncode, 0, bare.stderr)

        fake_gh = self.bin / "gh"
        fake_gh.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, sys\n"
            "with open(os.environ['FAKE_GH_CALLS'], 'a') as f: f.write(json.dumps(sys.argv[1:]) + '\\n')\n"
            "a = sys.argv[1:]\n"
            "if a[0] == 'api' and 'dispatches' in ' '.join(a):\n"
            "  with open(os.environ['FAKE_GH_PAYLOAD'], 'w') as f: f.write(sys.stdin.read())\n"
            "  print(json.dumps({'workflow_run_id': 314, 'html_url': 'https://github.com/quality-sh/provenance/actions/runs/314'}))\n"
            "elif a[:2] == ['run', 'list']:\n"
            "  branch = a[a.index('--branch') + 1]\n"
            "  with open(os.environ['FAKE_GH_PAYLOAD'], 'w') as f: json.dump({'inputs': {'command_key': branch.split('/')[1], 'command_json': ''}}, f)\n"
            "  print(json.dumps([{'databaseId': 314, 'headSha': branch.split('/')[2].split('-')[0], 'workflowName': 'Agent Cargo verification'}]))\n"
            "elif a[:2] == ['run', 'watch'] and '--compact' in a:\n"
            "  sys.exit('unsupported watch flag')\n"
            "elif a[:2] == ['run', 'view'] and '--json' in a:\n"
            "  result = os.environ.get('FAKE_TEST_RESULT', 'success')\n"
            "  inputs = json.load(open(os.environ['FAKE_GH_PAYLOAD']))['inputs']\n"
            "  names = ['Command'] if inputs.get('command_json') else [inputs['command_key'].capitalize()]\n"
            "  print(json.dumps({'jobs': [{'name': name, 'databaseId': i, 'conclusion': result if name in ('Test', 'Command') else 'success'} for i, name in enumerate(names, 1)]}))\n"
            "elif a[:2] == ['run', 'view'] and '--log-failed' in a:\n"
            "  if os.environ.get('FAKE_LOG_ERROR') == '1': sys.exit(1)\n"
            "  print('remote test failure')\n"
        )
        fake_gh.chmod(0o755)
        fake_cargo = self.bin / "cargo"
        fake_cargo.write_text('#!/bin/sh\necho LOCAL "$@"\n')
        fake_cargo.chmod(0o755)

        self.env = os.environ.copy()
        self.env.update(
            {
                "PATH": str(self.bin) + os.pathsep + self.env.get("PATH", ""),
                "FAKE_GH_CALLS": str(self.calls),
                "FAKE_GH_PAYLOAD": str(self.payload),
                "PROVENANCE_CI_REPO": "quality-sh/provenance",
                "PROVENANCE_CI_PUSH_URL": str(self.remote),
                "PROVENANCE_CI_STATE_DIR": str(self.root / "state"),
            }
        )

    def git(self, *args):
        result = run("git", *args, cwd=self.repo)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def cargo(self, *args, env=None, cwd=None):
        return run(sys.executable, str(WRAPPER), *args, cwd=cwd or self.repo, env=env or self.env)

    def calls_matching(self, word):
        if not self.calls.exists():
            return []
        return [json.loads(line) for line in self.calls.read_text().splitlines() if word in line]

    def test_consecutive_checks_use_one_snapshot_and_separate_requested_jobs(self):
        head = self.git("rev-parse", "HEAD")
        (self.repo / "src.rs").write_text("after\n")

        clippy = self.cargo("clippy")
        test = self.cargo("test")

        self.assertEqual(clippy.returncode, 0, clippy.stderr)
        self.assertEqual(test.returncode, 0, test.stderr)
        self.assertEqual(self.git("rev-parse", "HEAD"), head)
        self.assertEqual(self.git("diff", "--cached", "--name-only"), "")
        self.assertEqual((self.repo / "src.rs").read_text(), "after\n")
        self.assertIn("src.rs", self.git("diff", "--name-only"))
        self.assertEqual(len(self.calls_matching("list")), 2)
        refs = run("git", "for-each-ref", "--format=%(objectname)", "refs/heads/agent-ci", cwd=self.remote)
        self.assertEqual(refs.returncode, 0, refs.stderr)
        snapshot = refs.stdout.strip().splitlines()[0]
        self.assertEqual(len(snapshot), 40)
        committed = run("git", "show", f"{snapshot}:src.rs", cwd=self.remote)
        self.assertEqual(committed.stdout, "after\n")

    def test_each_command_gets_its_own_ci_result(self):
        env = {**self.env, "FAKE_TEST_RESULT": "failure"}

        clippy = self.cargo("clippy", env=env)
        test = self.cargo("test", env=env)

        self.assertEqual(clippy.returncode, 0, clippy.stderr)
        self.assertEqual(test.returncode, 1)
        self.assertIn("remote test failure", test.stdout)
        self.assertEqual(len(self.calls_matching("list")), 2)

    def test_repeated_check_for_same_snapshot_reuses_its_run(self):
        first = self.cargo("check")
        second = self.cargo("check")

        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(len(self.calls_matching("list")), 1)

    def test_non_verification_cargo_commands_run_locally(self):
        result = self.cargo("fmt", "--check")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("LOCAL fmt --check", result.stdout)
        self.assertEqual(self.calls_matching("dispatches"), [])

    def test_package_specific_test_runs_remotely_with_exact_arguments(self):
        result = self.cargo("test", "-p", "provenance-cli", "--", "--nocapture")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.payload.exists(), "The Cargo command did not dispatch to CI")
        payload = json.loads(self.payload.read_text())
        self.assertEqual(
            json.loads(payload["inputs"]["command_json"]),
            ["test", "-p", "provenance-cli", "--", "--nocapture"],
        )
        self.assertEqual(len(self.calls_matching("dispatches")), 1)

    def test_opt_in_path_intercepts_cargo_in_an_agent_shell(self):
        env = {
            **self.env,
            "PATH": str(WRAPPER.parent) + os.pathsep + self.env["PATH"],
        }
        result = run("cargo", "test", cwd=self.repo, env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Remote Cargo: test passed", result.stdout)
        self.assertEqual(len(self.calls_matching("list")), 1)

    def test_untracked_files_must_be_staged_before_a_public_snapshot(self):
        (self.repo / "secret.txt").write_text("not meant for CI\n")

        result = self.cargo("test")

        self.assertEqual(result.returncode, 2)
        self.assertIn("Untracked files", result.stderr)
        self.assertEqual(self.calls_matching("dispatches"), [])

    def test_staged_new_file_is_included_in_remote_snapshot(self):
        (self.repo / "new.rs").write_text("new source\n")
        self.git("add", "new.rs")

        result = self.cargo("check")

        self.assertEqual(result.returncode, 0, result.stderr)
        refs = run("git", "for-each-ref", "--format=%(objectname)", "refs/heads/agent-ci", cwd=self.remote)
        self.assertEqual(refs.returncode, 0, refs.stderr)
        committed = run("git", "show", f"{refs.stdout.strip()}:new.rs", cwd=self.remote)
        self.assertEqual(committed.stdout, "new source\n")

    def test_unrelated_repository_uses_native_cargo(self):
        unrelated = self.root / "unrelated"
        unrelated.mkdir()

        result = self.cargo("test", cwd=unrelated)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("LOCAL test", result.stdout)
        self.assertEqual(self.calls_matching("dispatches"), [])

    def test_failed_check_still_returns_failure_when_log_download_is_unavailable(self):
        env = {**self.env, "FAKE_TEST_RESULT": "failure", "FAKE_LOG_ERROR": "1"}
        result = self.cargo("test", env=env)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Remote Cargo: test failed", result.stdout)

    def test_explicit_local_override_uses_native_cargo(self):
        env = {**self.env, "PROVENANCE_CI_LOCAL": "1"}
        result = self.cargo("build", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("LOCAL build", result.stdout)
        self.assertEqual(self.calls_matching("dispatches"), [])


if __name__ == "__main__":
    unittest.main()
