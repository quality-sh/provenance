#!/usr/bin/env python3
"""Runs the stuck test report against process trees that this test starts.

Each test makes its own workspace root, a temporary directory, and gives it
to the script. The fake test binary is a copy of sh below
`<root>/target/debug/deps/`. It starts two `flock` processes on one file: the
first holds the lock and runs a fake server (a copy of sleep below
`<root>/target/debug/`) that waits, the second waits for the lock. The
watched name is the name of the fake server, which no other process on the
machine has, so the script cannot read or stop a process that the test did
not start.
"""

import importlib.util
import io
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

SCRIPT = Path(__file__).with_name("stuck_test_report.py")
LIMIT = 2
INTERVAL = 0.2
SERVER = "stuck_case_server"

# The import of the script writes no bytecode file into the checkout.
sys.dont_write_bytecode = True
_spec = importlib.util.spec_from_file_location("stuck_test_report", SCRIPT)
report_module = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(report_module)


def gone(pid):
    try:
        state = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1][0]
    except OSError:
        return True
    return state == "Z"


def wait_for(condition, seconds, what):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if condition():
            return
        time.sleep(0.1)
    raise AssertionError(f"{what} did not occur in {seconds} s")


def children(pid):
    found = []
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            try:
                stat = Path(f"/proc/{entry}/stat").read_text()
            except OSError:
                continue
            if int(stat.rsplit(") ", 1)[1].split()[1]) == pid:
                found.append(int(entry))
    return found


def stop_group(pid):
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


@unittest.skipUnless(sys.platform.startswith("linux"), "the report reads /proc")
class StuckTestReportTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp(prefix="stuck-test-report-"))
        self.addCleanup(shutil.rmtree, self.directory, ignore_errors=True)
        deps = self.directory / "target" / "debug" / "deps"
        deps.mkdir(parents=True)
        self.binary = deps / "stuck_case-0123456789abcdef"
        self.server = self.directory / "target" / "debug" / SERVER
        shutil.copy2(shutil.which("sh"), self.binary)
        shutil.copy2(shutil.which("sleep"), self.server)
        self.lock = self.directory / "repository.lock"
        self.lock.touch()
        self.marker = self.directory / "holds-the-lock"
        self.environment = {**os.environ, "RUNNER_TEMP": str(self.directory)}
        self.environment.pop("GITHUB_ACTIONS", None)

    def start_tree(self):
        script = (
            f'flock "{self.lock}" sh -c \'touch "$0"; exec "$1" 300\' "{self.marker}" "{self.server}" & '
            f'until [ -e "{self.marker}" ]; do :; done; '
            f'flock "{self.lock}" "{self.server}" 300 & wait'
        )
        # The script comes on standard input, so the command line looks like
        # the command line of a test binary: the test name and options.
        tree = subprocess.Popen(
            [str(self.binary), "-s", "waits_for_the_lock", "--exact"],
            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, text=True,
            start_new_session=True,
        )
        self.addCleanup(tree.wait)
        self.addCleanup(stop_group, tree.pid)
        tree.stdin.write(script + "\n")
        tree.stdin.close()
        return tree

    def script(self, *arguments):
        return subprocess.run(
            [sys.executable, str(SCRIPT), *arguments],
            cwd=self.directory, env=self.environment, text=True, capture_output=True, check=True,
        ).stdout

    def watcher(self, limit):
        """A watcher in the background; the test stops it, also when an assertion fails."""
        watcher = subprocess.Popen(
            [sys.executable, str(SCRIPT), "watch", "--limit", str(limit), "--watch", SERVER,
             "--root", str(self.directory), "--interval", str(INTERVAL)],
            env=self.environment, text=True, stdout=subprocess.PIPE,
        )
        self.addCleanup(watcher.wait)
        self.addCleanup(watcher.kill)
        return watcher

    def check_until_report(self, watcher):
        """Runs the checks of a watcher in this process until it writes a report.

        This process is the parent of the tree, so the watcher can read the
        system call of each thread also when it is not root.
        """
        deadline = time.monotonic() + 30
        while "::error" not in watcher.out.getvalue():
            self.assertLess(time.monotonic(), deadline, watcher.out.getvalue())
            watcher.check()
            time.sleep(INTERVAL)
        return watcher.out.getvalue()

    def watcher_here(self):
        return report_module.Watcher(LIMIT, [SERVER], str(self.directory), os.getuid(), False, io.StringIO())

    def started_tree(self):
        """The tree with its two flock processes and the fake server."""
        tree = self.start_tree()
        wait_for(lambda: len(children(tree.pid)) == 2, 10, "two flock processes")
        flocks = sorted(children(tree.pid))
        wait_for(lambda: any(children(pid) for pid in flocks), 10, "the fake server")
        holder = next(pid for pid in flocks if children(pid))
        waiter = next(pid for pid in flocks if pid != holder)
        return tree, holder, waiter, children(holder)[0]

    def test_a_fault_of_the_report_does_not_prevent_the_stop(self):
        tree, holder, waiter, server = self.started_tree()
        fault = RuntimeError("a fault of the lock section")
        with mock.patch.object(report_module.Watcher, "write_locks", side_effect=fault):
            report = self.check_until_report(self.watcher_here())
        self.assertEqual(tree.wait(timeout=10), -signal.SIGKILL)
        wait_for(lambda: all(gone(pid) for pid in (holder, waiter, server)), 10, "the end of the tree")
        # The report keeps what it wrote before the fault, gives the fault,
        # and then the error line and the stop follow.
        self.assertRegex(report, rf"process {server}, parent {holder}, ")
        self.assertRegex(
            report,
            r"The report is not complete: RuntimeError\('a fault of the lock section'\) at line \d+ \(\w+\)\. "
            r"The watcher stops the tree\.\n::endgroup::\n::error title=Stuck test stuck_case waits_for_the_lock::"
            r".*\nThe watcher killed the tree of 4 processes\.\n$",
        )
        self.assertNotIn("locks on the open files", report)

    def test_a_process_that_ends_during_the_report_does_not_stop_the_report(self):
        # The values of this process, with the id of a process that ended.
        ended = subprocess.Popen(["true"])
        ended.wait()
        watcher = self.watcher_here()
        process = report_module.Process(os.getpid())
        process.pid = ended.pid
        watcher.describe(process, 0, report_module.uptime(), {})
        report = watcher.out.getvalue()
        self.assertIn("threads: (No such file or directory)", report)
        self.assertIn("open files: (No such file or directory)", report)
        self.assertIn("listening ports: none", report)

    def test_a_stuck_test_gets_one_report_and_its_tree_is_stopped(self):
        tree = self.start_tree()
        wait_for(lambda: len(children(tree.pid)) == 2, 10, "two flock processes")
        flocks = sorted(children(tree.pid))
        wait_for(lambda: any(children(pid) for pid in flocks), 10, "the fake server")
        holder = next(pid for pid in flocks if children(pid))
        waiter = next(pid for pid in flocks if pid != holder)
        server = children(holder)[0]

        self.script(
            "start", "--limit", str(LIMIT), "--watch", SERVER, "--root", str(self.directory), "--interval", str(INTERVAL)
        )
        try:
            self.assertEqual(tree.wait(timeout=30), -signal.SIGKILL)
            wait_for(lambda: all(gone(pid) for pid in (holder, waiter, server)), 10, "the end of the tree")
        finally:
            # The job stops the watcher when the test step ends. When a wait
            # above fails, this stops the watcher before the test fails.
            report = self.script("finish")

        self.assertEqual(report.count("::group::Stuck test: stuck_case waits_for_the_lock"), 1, report)
        self.assertIn(f"(process {tree.pid}, ", report)
        self.assertEqual(report.count("::error "), 1, report)
        self.assertRegex(
            report,
            r"::error title=Stuck test stuck_case waits_for_the_lock::The test stuck_case waits_for_the_lock "
            rf"ran \d+ s, past the limit of {LIMIT} s\. .*The watcher stops its process tree\.\n"
            r"The watcher killed the tree of 4 processes\.\n",
        )
        self.assertNotIn("No test ran past the limit.", report)
        self.assertIn("::endgroup::", report)
        for pid, parent, program in (
            (tree.pid, os.getpid(), self.binary),
            (holder, tree.pid, shutil.which("flock")),
            (waiter, tree.pid, shutil.which("flock")),
            (server, holder, self.server),
        ):
            self.assertRegex(report, rf"process {pid}, parent {parent}, age \d+ s, state S, program {program}\n")
        self.assertIn(f"command line: {self.binary} -s waits_for_the_lock --exact\n", report)
        self.assertIn(f"command line: {self.server} 300\n", report)
        # Each thread shows its wait. The wait channel is readable for each
        # process of the same user; the system call only for root or a parent.
        self.assertRegex(report, rf"thread {tree.pid} stuck_case-0123: state S, wait channel \w+, system call ")
        self.assertRegex(report, rf"thread {server} {SERVER[:15]}: state S, wait channel hrtimer_nanosleep, system call ")
        self.assertRegex(report, rf"thread {waiter} flock: state S, wait channel \w*lock\w*, system call ")
        self.assertRegex(report, rf"file 3: {self.lock}")
        self.assertIn("listening ports: none", report)
        self.assertRegex(report, rf"process {holder}: flock .* holds a FLOCK WRITE lock on {self.lock}")
        self.assertRegex(report, rf"process {waiter}: flock .* waits for a FLOCK WRITE lock on {self.lock}")

    def test_a_young_test_and_a_watched_name_outside_the_root_get_no_report(self):
        # A process with the watched name whose program is outside the root,
        # in a session of its own: the script does not read it or stop it.
        outside = self.directory / "outside"
        outside.mkdir()
        stranger = subprocess.Popen(
            [shutil.copy2(shutil.which("sleep"), outside / SERVER), "300"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True,
        )
        self.addCleanup(stranger.wait)
        self.addCleanup(stranger.kill)
        time.sleep(LIMIT + 1.5)
        tree = self.start_tree()
        watcher = self.watcher(LIMIT + 1)
        time.sleep(1.5)
        watcher.terminate()
        output = watcher.communicate(timeout=10)[0]
        self.assertIn(f"Watching the processes below {os.path.realpath(self.directory)}/target/ with a limit of 3 s", output)
        self.assertNotIn("::group::", output)
        self.assertNotIn("::error", output)
        self.assertNotIn(str(outside), output)
        self.assertNotIn(f"process {stranger.pid}", output)
        self.assertIsNone(tree.poll())
        self.assertIsNone(stranger.poll())


if __name__ == "__main__":
    unittest.main()
