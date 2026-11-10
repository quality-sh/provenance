import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


WRAPPER = Path(__file__).resolve().parents[2] / "scripts" / "agent-bin" / "cargo"
CLOCK_START = 1_800_000_000

# A fake gh. FAKE_API_RUNS is a JSON list of the answers that the run read gives
# in sequence (the last answer repeats): in_progress, completed, 500, 404, or
# rate:<seconds until reset>. "gh run watch" reads the same sequence and stops
# with an error at the first error answer, as the real gh does.
FAKE_GH = r"""#!/usr/bin/env python3
import json, os, re, sys
from pathlib import Path

calls = os.environ['FAKE_GH_CALLS']
with open(calls, 'a') as f: f.write(json.dumps(sys.argv[1:]) + '\n')
a = sys.argv[1:]
state = Path(os.environ['FAKE_GH_STATE'])
state.mkdir(exist_ok=True)

def record(run_id):
    return json.loads((state / f'run-{run_id}.json').read_text())

def answer(run_id):
    counter = state / f'reads-{run_id}'
    count = int(counter.read_text()) if counter.exists() else 0
    counter.write_text(str(count + 1))
    answers = json.loads(os.environ.get('FAKE_API_RUNS', '["completed"]'))
    return answers[min(count, len(answers) - 1)]

def respond(status, headers, body):
    reason = {200: 'OK', 403: 'Forbidden', 404: 'Not Found', 500: 'Internal Server Error'}[status]
    lines = [f'HTTP/2.0 {status} {reason}', 'Content-Type: application/json'] + [f'{k}: {v}' for k, v in headers.items()]
    sys.stdout.write('\r\n'.join(lines) + '\r\n\r\n' + json.dumps(body))
    if status != 200:
        sys.stderr.write(f'gh: {body["message"]} (HTTP {status})\n')
        sys.exit(1)

def run_answer(run_id, value):
    if value == '500':
        respond(500, {}, {'message': 'Server Error'})
    if value == '404':
        respond(404, {}, {'message': 'Not Found'})
    if value.startswith('rate:'):
        reset = int(os.environ['FAKE_NOW']) + int(value.split(':')[1])
        respond(403, {'X-Ratelimit-Remaining': '0', 'X-Ratelimit-Reset': str(reset)}, {'message': 'API rate limit exceeded'})
    run = record(run_id)
    respond(200, {}, {'id': int(run_id), 'status': value, 'conclusion': 'success' if value == 'completed' else None,
                      'head_branch': run['branch'], 'head_sha': run['sha'],
                      'html_url': f'https://github.com/quality-sh/provenance/actions/runs/{run_id}'})

def job_names(run):
    return ['Command'] if run['command_json'] else [run['command_key'].split('-')[0].capitalize()]

def conclusion(name):
    if name == 'Build':
        return os.environ.get('FAKE_BUILD_RESULT', 'success')
    return os.environ.get('FAKE_TEST_RESULT', 'success') if name in ('Test', 'Command') else 'success'

if a[0] == 'api' and 'dispatches' in ' '.join(a):
    payload = json.loads(sys.stdin.read())
    with open(os.environ['FAKE_GH_PAYLOAD'], 'w') as f: json.dump(payload, f)
    run_id = 314 + len(list(state.glob('run-*.json')))
    inputs = payload['inputs']
    (state / f'run-{run_id}.json').write_text(json.dumps({'branch': payload['ref'], 'sha': inputs['snapshot_sha'], 'command_key': inputs['command_key'], 'command_json': inputs['command_json']}))
    print(json.dumps({'workflow_run_id': run_id, 'html_url': f'https://github.com/quality-sh/provenance/actions/runs/{run_id}'}))
elif a[:2] == ['run', 'list']:
    branch = a[a.index('--branch') + 1]
    lists = sum(1 for line in open(calls) if line.startswith('["run", "list"'))
    if lists <= int(os.environ.get('FAKE_RUN_LIST_MISSES', '0')):
        print('[]'); sys.exit(0)
    sha = branch.split('/')[2].split('-')[0]
    with open(os.environ['FAKE_GH_PAYLOAD'], 'w') as f: json.dump({'headBranch': branch, 'headSha': sha, 'inputs': {'command_key': branch.split('/')[1], 'command_json': ''}}, f)
    (state / 'run-314.json').write_text(json.dumps({'branch': branch, 'sha': sha, 'command_key': branch.split('/')[1], 'command_json': ''}))
    print(json.dumps([{'databaseId': 314, 'headSha': sha, 'workflowName': 'Agent Cargo verification'}]))
elif a[0] == 'api' and '-i' in a and re.fullmatch(r'repos/[^/]+/[^/]+/actions/runs/\d+', a[-1]):
    run_id = a[-1].rsplit('/', 1)[1]
    run_answer(run_id, answer(run_id))
elif a[0] == 'api' and '-i' in a and re.fullmatch(r'repos/[^/]+/[^/]+/actions/runs/\d+/jobs(\?.*)?', a[-1]):
    run = record(a[-1].split('/')[5])
    respond(200, {}, {'jobs': [{'name': name, 'id': i, 'conclusion': conclusion(name)} for i, name in enumerate(job_names(run), 1)]})
elif a[0] == 'api' and re.fullmatch(r'repos/[^/]+/[^/]+/actions/jobs/\d+/logs', a[-1]):
    if os.environ.get('FAKE_JOB_LOG_ERROR') == '1': sys.exit('gh: Not Found (HTTP 404)')
    log = os.environ.get('FAKE_JOB_LOG')
    sys.stdout.write(Path(log).read_text() if log else
        '2026-11-10T00:00:01Z running 3 tests\n'
        '2026-11-10T00:00:02Z test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
        '2026-11-10T00:00:03Z test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.01s\n')
elif a[:2] == ['run', 'watch']:
    if '--compact' in a: sys.exit('unsupported watch flag')
    while True:
        value = answer(a[2])
        if value != 'in_progress' and value != 'completed':
            sys.exit(f'failed to get run: HTTP {value.split(":")[0].replace("rate", "403")}')
        if value == 'completed':
            break
elif a[:2] == ['run', 'view'] and '--json' in a:
    run = record(a[2])
    print(json.dumps({'headBranch': run['branch'], 'headSha': run['sha'], 'status': 'completed', 'jobs': [{'name': name, 'databaseId': i, 'conclusion': conclusion(name)} for i, name in enumerate(job_names(run), 1)]}))
elif a[:2] == ['run', 'view'] and '--log-failed' in a:
    if os.environ.get('FAKE_LOG_ERROR') == '1': sys.exit(1)
    print('remote test failure')
elif a[:2] == ['run', 'download']:
    dest = Path(a[a.index('--dir') + 1])
    dest.mkdir(parents=True, exist_ok=True)
    broken = os.environ.get('FAKE_BINARY_BROKEN') == '1'
    (dest / 'provenance').write_text('#!/bin/sh\n' + ('exit 1\n' if broken else 'echo provenance 0.2.2\n'))
else:
    sys.exit(f'fake gh: unexpected call {a}')
"""

# Runs the shim with a fake clock: time.sleep records the wait and moves the
# clock, so a test of a long watch takes no real time.
CLOCK = r"""
import json, os, runpy, sys, time
now = [float(os.environ['FAKE_NOW'])]
def sleep(seconds):
    with open(os.environ['FAKE_GH_CALLS'], 'a') as f: f.write(json.dumps(['sleep', seconds]) + '\n')
    now[0] += seconds
    os.environ['FAKE_NOW'] = str(int(now[0]))
time.sleep = sleep
time.time = lambda: now[0]
time.monotonic = lambda: now[0]
sys.argv = sys.argv[1:]
runpy.run_path(sys.argv[0], run_name='__main__')
"""


def run(*args, cwd, env=None):
    # Cases that load the shim as a module must not write bytecode next to it:
    # the shim refuses a snapshot when the worktree has an untracked file.
    env = {**(os.environ if env is None else env), "PYTHONDONTWRITEBYTECODE": "1"}
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
        workflow = self.repo / ".github/workflows/agent-cargo.yml"
        workflow.parent.mkdir(parents=True)
        workflow.write_text("name: Agent Cargo verification\n")
        (self.repo / "src.rs").write_text("before\n")
        self.git("add", ".")
        self.git("commit", "-m", "base")
        bare = run("git", "init", "--bare", str(self.remote), cwd=self.root)
        self.assertEqual(bare.returncode, 0, bare.stderr)

        fake_gh = self.bin / "gh"
        fake_gh.write_text(FAKE_GH)
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
                "FAKE_GH_STATE": str(self.root / "gh-state"),
                "FAKE_NOW": str(CLOCK_START),
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

    def cargo_with_clock(self, *args, env=None):
        return run(sys.executable, "-c", CLOCK, str(WRAPPER), *args, cwd=self.repo, env=env or self.env)

    def sleeps(self):
        return [call[1] for call in self.calls_matching("sleep") if call[0] == "sleep"]

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
        snapshot = clippy.stdout.split("Remote Cargo: verifying snapshot ", 1)[1].splitlines()[0]
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

    def test_completed_check_removes_its_remote_branch(self):
        result = self.cargo("check")

        self.assertEqual(result.returncode, 0, result.stderr)
        refs = run("git", "for-each-ref", "--format=%(refname)", "refs/heads/agent-ci", cwd=self.remote)
        self.assertEqual(refs.returncode, 0, refs.stderr)
        self.assertEqual(refs.stdout.strip(), "")

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

    def test_old_worktree_without_agent_workflow_uses_native_cargo(self):
        (self.repo / ".github/workflows/agent-cargo.yml").unlink()

        result = self.cargo("test")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("LOCAL test", result.stdout)
        self.assertEqual(self.calls_matching("list"), [])

    def test_native_cargo_resolution_ignores_an_installed_shim_symlink(self):
        linkbin = self.root / "linkbin"
        linkbin.mkdir()
        (linkbin / "cargo").symlink_to(WRAPPER)
        env = {**self.env, "PATH": str(linkbin) + os.pathsep + self.env["PATH"]}
        script = (
            "import runpy; "
            f"print(runpy.run_path({str(WRAPPER)!r})['real_cargo']())"
        )

        result = run(sys.executable, "-c", script, cwd=self.repo, env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(result.stdout.strip()), self.bin / "cargo")

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
        snapshot = result.stdout.split("Remote Cargo: verifying snapshot ", 1)[1].splitlines()[0]
        committed = run("git", "show", f"{snapshot}:new.rs", cwd=self.remote)
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

    def test_opt_in_build_downloads_a_usable_binary(self):
        env = {**self.env, "PROVENANCE_CI_ARTIFACT": "1"}

        result = self.cargo("build", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("binary available", result.stdout)
        binary = self.repo / "target/debug/provenance"
        self.assertEqual(run(str(binary), "--version", cwd=self.repo).stdout.strip(), "provenance 0.2.2")
        self.assertEqual(len(self.calls_matching("download")), 1)
        branches = [call[call.index("--branch") + 1] for call in self.calls_matching("list")]
        self.assertTrue(any(branch.startswith("agent-ci/build-artifact/") for branch in branches))

    def test_check_after_an_artifact_download_snapshots_the_change(self):
        (self.repo / ".gitignore").write_text("target/\n")
        self.git("add", ".gitignore")
        self.git("commit", "-m", "ignore target")
        artifact = self.cargo("build", env={**self.env, "PROVENANCE_CI_ARTIFACT": "1"})
        self.assertEqual(artifact.returncode, 0, artifact.stderr)
        self.assertTrue((self.repo / "target/debug/provenance").is_file())
        (self.repo / "src.rs").write_text("after\n")

        result = self.cargo("check")

        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(self.payload.read_text())
        snapshot = payload["headSha"]
        self.assertEqual(self.git("show", f"{snapshot}:src.rs"), "after")
        self.assertEqual(self.git("ls-tree", "-r", "--name-only", snapshot, "--", "target"), "")

    def check_snapshot_entries(self, ignore_target):
        if ignore_target:
            (self.repo / ".gitignore").write_text("target/\n")
            self.git("add", ".gitignore")
            self.git("commit", "-m", "ignore target")
        (self.repo / "src.rs").write_text("after\n")

        result = self.cargo("check")

        self.assertEqual(result.returncode, 0, result.stderr)
        snapshot = json.loads(self.payload.read_text())["headSha"]
        self.assertEqual(self.git("show", f"{snapshot}:src.rs"), "after")
        return self.git("ls-tree", "-r", "--name-only", snapshot).splitlines()

    def test_snapshot_leaves_out_a_target_directory(self):
        built = self.repo / "target/debug/provenance"
        built.parent.mkdir(parents=True)
        built.write_text("local binary")

        entries = self.check_snapshot_entries(ignore_target=False)

        self.assertEqual(sorted(entries), [".github/workflows/agent-cargo.yml", "Cargo.toml", "src.rs"], entries)

    def test_snapshot_leaves_out_a_target_symlink(self):
        shared = self.root / "shared-target"
        shared.mkdir()
        (self.repo / "target").symlink_to(shared, target_is_directory=True)

        entries = self.check_snapshot_entries(ignore_target=True)

        self.assertEqual(sorted(entries), [".github/workflows/agent-cargo.yml", ".gitignore", "Cargo.toml", "src.rs"], entries)

    def test_snapshot_leaves_out_a_target_file(self):
        (self.repo / "target").write_text("local file")

        entries = self.check_snapshot_entries(ignore_target=True)

        self.assertEqual(sorted(entries), [".github/workflows/agent-cargo.yml", ".gitignore", "Cargo.toml", "src.rs"], entries)

    def test_snapshot_without_target_holds_the_change(self):
        entries = self.check_snapshot_entries(ignore_target=True)

        self.assertEqual(sorted(entries), [".github/workflows/agent-cargo.yml", ".gitignore", "Cargo.toml", "src.rs"], entries)

    def test_shared_target_symlink_keeps_binary_in_worktree_git_dir(self):
        shared = self.root / "shared-target"
        shared_binary = shared / "debug/provenance"
        shared_binary.parent.mkdir(parents=True)
        shared_binary.write_text("another worktree's binary")
        (self.repo / "target").symlink_to(shared, target_is_directory=True)
        env = {**self.env, "PROVENANCE_CI_ARTIFACT": "1"}

        result = self.cargo("build", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(shared_binary.read_text(), "another worktree's binary")
        installed = self.repo / ".git/agent-cargo/bin"
        binaries = list(installed.glob("*/provenance"))
        self.assertEqual(len(binaries), 1)
        self.assertEqual(run(str(binaries[0]), "--version", cwd=self.repo).stdout.strip(), "provenance 0.2.2")

    def test_normal_build_does_not_download_an_artifact(self):
        result = self.cargo("build")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls_matching("download"), [])
        self.assertFalse((self.repo / "target/debug/provenance").exists())

    def test_opt_in_build_does_not_reuse_a_plain_build_without_an_artifact(self):
        ordinary = self.cargo("build")
        artifact = self.cargo("build", env={**self.env, "PROVENANCE_CI_ARTIFACT": "1"})

        self.assertEqual(ordinary.returncode, 0, ordinary.stderr)
        self.assertEqual(artifact.returncode, 0, artifact.stderr)
        self.assertEqual(len(self.calls_matching("list")), 2)
        self.assertEqual(len(self.calls_matching("download")), 1)

    def test_failed_opt_in_build_does_not_download_or_replace_binary(self):
        binary = self.repo / "target/debug/provenance"
        binary.parent.mkdir(parents=True)
        binary.write_text("previous binary")
        env = {**self.env, "PROVENANCE_CI_ARTIFACT": "1", "FAKE_BUILD_RESULT": "failure"}

        result = self.cargo("build", env=env)

        self.assertEqual(result.returncode, 1)
        self.assertEqual(binary.read_text(), "previous binary")
        self.assertEqual(self.calls_matching("download"), [])

    def test_unusable_download_does_not_replace_existing_binary(self):
        binary = self.repo / "target/debug/provenance"
        binary.parent.mkdir(parents=True)
        binary.write_text("previous binary")
        env = {**self.env, "PROVENANCE_CI_ARTIFACT": "1", "FAKE_BINARY_BROKEN": "1"}

        result = self.cargo("build", env=env)

        self.assertEqual(result.returncode, 2)
        self.assertEqual(binary.read_text(), "previous binary")

    def test_watch_reads_the_run_every_30_seconds(self):
        env = {**self.env, "FAKE_API_RUNS": '["in_progress", "in_progress", "completed"]'}

        result = self.cargo_with_clock("check", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls_matching("watch"), [])
        self.assertEqual(len(self.calls_matching("actions/runs/314\"")), 3)
        self.assertEqual(self.sleeps(), [30, 30])

    def test_run_search_sleeps_10_seconds_between_list_calls(self):
        env = {**self.env, "FAKE_RUN_LIST_MISSES": "2"}

        result = self.cargo_with_clock("check", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        search = [
            call[0] if call[0] == "sleep" else " ".join(call[:2])
            for call in (json.loads(line) for line in self.calls.read_text().splitlines())
            if call[0] == "sleep" or call[:2] == ["run", "list"]
        ]
        self.assertEqual(search, ["run list", "sleep", "run list", "sleep", "run list"])
        self.assertEqual(self.sleeps(), [10, 10])

    def test_api_errors_during_the_watch_do_not_stop_it(self):
        env = {**self.env, "FAKE_API_RUNS": '["in_progress", "500", "404", "in_progress", "completed"]'}

        result = self.cargo_with_clock("check", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Remote Cargo: check passed", result.stdout)
        self.assertIn("HTTP 500", result.stderr)
        self.assertIn("not found", result.stderr)
        self.assertEqual(self.sleeps(), [30, 30, 30, 30])

    def test_consecutive_api_errors_back_off(self):
        env = {**self.env, "FAKE_API_RUNS": '["500", "500", "500", "completed"]'}

        result = self.cargo_with_clock("check", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.sleeps(), [30, 60, 120])

    def test_rate_limit_waits_until_the_reset_time(self):
        env = {**self.env, "FAKE_API_RUNS": '["in_progress", "rate:200", "completed"]'}

        result = self.cargo_with_clock("check", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.sleeps(), [30, 201])
        self.assertIn("rate limit", result.stderr)
        self.assertIn("201 s", result.stderr)

    def test_rate_limit_wait_has_an_upper_limit(self):
        env = {**self.env, "FAKE_API_RUNS": '["rate:7200", "completed"]'}

        result = self.cargo_with_clock("check", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.sleeps(), [900])

    def assert_result_not_known(self, result, run_id="314"):
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn(f"https://github.com/quality-sh/provenance/actions/runs/{run_id}", result.stderr)
        self.assertIn(f"gh run view {run_id} --repo quality-sh/provenance", result.stderr)

    def test_run_that_is_never_found_gives_up_with_its_url(self):
        env = {**self.env, "FAKE_API_RUNS": '["404"]'}

        result = self.cargo_with_clock("check", env=env)

        self.assert_result_not_known(result)
        self.assertIn("not found", result.stderr)
        self.assertEqual(self.sleeps(), [30] * 10)

    def test_api_errors_give_up_after_the_total_limit(self):
        env = {**self.env, "FAKE_API_RUNS": '["500"]'}

        result = self.cargo_with_clock("check", env=env)

        self.assert_result_not_known(result)
        self.assertEqual(self.sleeps(), [30, 60, 120, 240, 300, 300, 300, 300, 300])

    def test_run_that_never_appears_after_the_push_is_not_known(self):
        env = {**self.env, "FAKE_RUN_LIST_MISSES": "1000"}

        result = self.cargo_with_clock("check", env=env)

        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("agent-ci/check/", result.stderr)
        self.assertIn("https://github.com/quality-sh/provenance/actions?query=branch%3A", result.stderr)

    def test_success_prints_the_run_and_the_test_counts(self):
        result = self.cargo_with_clock("test", "-p", "provenance-macros")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("https://github.com/quality-sh/provenance/actions/runs/314", result.stdout)
        self.assertIn("job Command: success", result.stdout)
        self.assertIn("5 passed, 0 failed, 1 ignored in 2 test binaries", result.stdout)
        self.assertIn("gh run view 314 --repo quality-sh/provenance --job 1 --log", result.stdout)
        self.assertEqual(len(self.calls_matching("/logs")), 1)

    def test_test_counts_skip_a_large_job_log(self):
        log = self.root / "large.log"
        log.write_text("x" * (5 * 1024 * 1024))
        env = {**self.env, "FAKE_JOB_LOG": str(log)}

        result = self.cargo_with_clock("test", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("larger than 4 MB", result.stdout)
        self.assertIn("gh run view 314 --repo quality-sh/provenance --job 1 --log", result.stdout)

    def test_check_command_reads_no_job_log(self):
        result = self.cargo_with_clock("check")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls_matching("/logs"), [])

    def concurrent_calls(self, *args, separate_clones):
        # A pre-receive hook holds each push until two pushes arrive, so both
        # calls push at the same time, as two workers on one commit do.
        barrier = self.root / "barrier"
        barrier.mkdir()
        hook = self.remote / "hooks/pre-receive"
        hook.write_text(
            "#!/bin/sh\n"
            f"touch {barrier}/$$\n"
            "for i in $(seq 100); do\n"
            f"  [ \"$(ls {barrier} | wc -l)\" -ge 2 ] && exit 0\n"
            "  sleep 0.1\n"
            "done\n"
        )
        hook.chmod(0o755)
        envs = [
            {**self.env, "PROVENANCE_CI_STATE_DIR": str(self.root / (f"state-{n}" if separate_clones else "state"))}
            for n in range(2)
        ]
        processes = [
            subprocess.Popen(
                [sys.executable, "-c", CLOCK, str(WRAPPER), *args], cwd=self.repo,
                env={**env, "PYTHONDONTWRITEBYTECODE": "1"}, text=True,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            for env in envs
        ]
        return [(process.wait(timeout=60), *process.communicate()) for process in processes]

    def test_two_calls_on_one_commit_from_two_clones_each_watch_their_own_run(self):
        results = self.concurrent_calls("test", "-p", "provenance-macros", separate_clones=True)

        for code, stdout, stderr in results:
            self.assertEqual(code, 0, stderr)
        dispatches = [call for call in self.calls_matching("dispatches")]
        self.assertEqual(len(dispatches), 2)
        urls = sorted(next(line for line in stdout.splitlines() if "/actions/runs/" in line) for _, stdout, _ in results)
        self.assertIn("/runs/314", urls[0])
        self.assertIn("/runs/315", urls[1])

    def test_two_calls_on_one_commit_in_one_clone_share_one_run(self):
        results = self.concurrent_calls("test", "-p", "provenance-macros", separate_clones=False)

        for code, stdout, stderr in results:
            self.assertEqual(code, 0, stderr)
            self.assertIn("/actions/runs/314", stdout)
        self.assertEqual(len(self.calls_matching("dispatches")), 1)

    def test_dispatch_snapshot_branch_is_removed_after_the_run(self):
        result = self.cargo_with_clock("test", "-p", "provenance-macros")

        self.assertEqual(result.returncode, 0, result.stderr)
        refs = run("git", "for-each-ref", "--format=%(refname)", "refs/heads/agent-snapshot", cwd=self.remote)
        self.assertEqual(refs.stdout.strip(), "")

    def test_artifact_opt_in_requires_plain_build(self):
        env = {**self.env, "PROVENANCE_CI_ARTIFACT": "1"}

        result = self.cargo("test", env=env)

        self.assertEqual(result.returncode, 2)
        self.assertIn("cargo build", result.stderr)
        self.assertEqual(self.calls_matching("list"), [])


if __name__ == "__main__":
    unittest.main()
