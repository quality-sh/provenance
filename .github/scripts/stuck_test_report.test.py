#!/usr/bin/env python3
"""Runs the stuck test report against process trees that this test starts.

The fake test binary is a copy of sh below a `target/` directory. It starts
two `flock` processes on one file: the first holds the lock and runs a fake
`provenance` (a copy of sleep) that waits, the second waits for the lock.
"""

import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPT = Path(__file__).with_name("stuck_test_report.py")
LIMIT = 2
INTERVAL = 0.2


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
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.directory, ignore_errors=True)
        deps = self.directory / "target" / "debug" / "deps"
        deps.mkdir(parents=True)
        self.binary = deps / "stuck_case-0123456789abcdef"
        self.provenance = self.directory / "target" / "debug" / "provenance"
        shutil.copy2(shutil.which("sh"), self.binary)
        shutil.copy2(shutil.which("sleep"), self.provenance)
        self.lock = self.directory / "repository.lock"
        self.lock.touch()
        self.marker = self.directory / "holds-the-lock"
        self.environment = {**os.environ, "RUNNER_TEMP": str(self.directory)}
        self.environment.pop("GITHUB_ACTIONS", None)

    def start_tree(self):
        script = (
            f'flock "{self.lock}" sh -c \'touch "$0"; exec "$1" 300\' "{self.marker}" "{self.provenance}" & '
            f'until [ -e "{self.marker}" ]; do :; done; '

            f'flock "{self.lock}" "{self.provenance}" 300 & wait'
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

    def test_a_stuck_test_gets_one_report_and_its_tree_is_stopped(self):
        tree = self.start_tree()
        wait_for(lambda: len(children(tree.pid)) == 2, 10, "two flock processes")
        flocks = sorted(children(tree.pid))
        wait_for(lambda: any(children(pid) for pid in flocks), 10, "the fake provenance")
        holder = next(pid for pid in flocks if children(pid))
        waiter = next(pid for pid in flocks if pid != holder)
        provenance = children(holder)[0]

        self.script("start", "--limit", str(LIMIT), "--watch", "provenance", "--interval", str(INTERVAL))
        self.assertEqual(tree.wait(timeout=30), -signal.SIGKILL)
        wait_for(lambda: all(gone(pid) for pid in (holder, waiter, provenance)), 10, "the end of the tree")
        # The job stops the watcher as soon as the test step ends.
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
            (provenance, holder, self.provenance),
        ):
            self.assertRegex(report, rf"process {pid}, parent {parent}, age \d+ s, state S, program {program}\n")
        self.assertIn(f"command line: {self.binary} -s waits_for_the_lock --exact\n", report)
        self.assertIn(f"command line: {self.provenance} 300\n", report)
        # Each thread shows its wait. The wait channel is readable for each
        # process of the same user; the system call only for root or a parent.
        self.assertRegex(report, rf"thread {tree.pid} stuck_case-0123: state S, wait channel \w+, system call ")
        self.assertRegex(report, rf"thread {provenance} provenance: state S, wait channel hrtimer_nanosleep, system call ")
        self.assertRegex(report, rf"thread {waiter} flock: state S, wait channel \w*lock\w*, system call ")
        self.assertRegex(report, rf"file 3: {self.lock}")
        self.assertIn("listening ports: none", report)
        self.assertRegex(report, rf"process {holder}: flock .* holds a FLOCK WRITE lock on {self.lock}")
        self.assertRegex(report, rf"process {waiter}: flock .* waits for a FLOCK WRITE lock on {self.lock}")

    def test_a_young_test_and_an_unwatched_old_process_get_no_report(self):
        outside = self.directory / "outside"
        outside.mkdir()
        unwatched = subprocess.Popen(
            [shutil.copy2(shutil.which("sleep"), outside / "helper"), "300"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        self.addCleanup(unwatched.wait)
        self.addCleanup(unwatched.kill)
        time.sleep(LIMIT + 1.5)
        tree = self.start_tree()
        watcher = subprocess.Popen(
            [sys.executable, str(SCRIPT), "watch", "--limit", str(LIMIT + 1), "--watch", "provenance",
             "--interval", str(INTERVAL)],
            cwd=self.directory, env=self.environment, text=True, stdout=subprocess.PIPE,
        )
        time.sleep(1.5)
        watcher.terminate()
        output = watcher.communicate(timeout=10)[0]
        self.assertIn("with a limit of 3 s", output)
        self.assertNotIn("::group::", output)
        self.assertIsNone(tree.poll())
        self.assertIsNone(unwatched.poll())


if __name__ == "__main__":
    unittest.main()
