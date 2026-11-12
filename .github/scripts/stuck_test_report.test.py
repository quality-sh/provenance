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
import re
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


def pids_of(program):
    """The processes whose program file is `program`."""
    found = []
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            try:
                if os.readlink(f"/proc/{entry}/exe") == str(program):
                    found.append(int(entry))
            except OSError:
                continue
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

    def fake_tool(self, body):
        """A fake eu-stack: a shell script with the given body."""
        tools = self.directory / "tools"
        tools.mkdir(exist_ok=True)
        tool = tools / "eu-stack"
        tool.write_text("#!/bin/sh\n" + body + "\n")
        tool.chmod(0o755)
        return str(tool)

    def test_a_backtrace_tool_that_does_not_end_stops_at_the_report_deadline(self):
        tree, holder, waiter, server = self.started_tree()
        tool = self.fake_tool("sleep 300 & wait")
        deadline = 2
        with mock.patch.object(report_module, "find_tool", return_value=tool), \
                mock.patch.object(report_module, "REPORT_DEADLINE", deadline):
            watcher = self.watcher_here()
            # The tree is old enough for a report at the first check.
            wait_for(lambda: report_module.Process(tree.pid).age(report_module.uptime()) > LIMIT, 10, "the limit")
            began = time.monotonic()
            watcher.check()
            took = time.monotonic() - began
        report = watcher.out.getvalue()
        self.assertEqual(tree.wait(timeout=10), -signal.SIGKILL)
        # The report deadline, plus the reads of /proc and the stop.
        self.assertLess(took, deadline + 8, report)
        self.assertIn(f"user backtrace of process {tree.pid} (eu-stack):\n", report)
        self.assertIn(f"  (eu-stack did not end before the report deadline of {deadline} s)\n", report)
        # The tree order: each parent before its children, brothers by id.
        rest = [pid for flock in sorted((holder, waiter)) for pid in ((holder, server) if flock == holder else (waiter,))]
        self.assertIn(
            f"The user backtraces are not complete: the report deadline of {deadline} s passed "
            f"before the backtrace of process {', '.join(map(str, rest))}.\n",
            report,
        )
        self.assertRegex(report, r"\n::endgroup::\n::error title=Stuck test .*\nThe watcher killed the tree of 4 processes\.\n$")

    def test_a_backtrace_gives_readable_rust_names(self):
        if not shutil.which("c++filt"):
            self.skipTest("binutils gives c++filt")
        tree, holder, waiter, server = self.started_tree()
        tool = self.fake_tool(
            'echo "PID $2 - process"; '
            "echo '  #1  0x00005619364c0976 _RNvMs_NtNtCs8ZKwes1zzDd_3std6thread6threadNtB4_6Thread4park'"
        )
        with mock.patch.object(report_module, "find_tool", return_value=tool):
            report = self.check_until_report(self.watcher_here())
        self.assertEqual(tree.wait(timeout=10), -signal.SIGKILL)
        for pid in (tree.pid, holder, waiter, server):
            self.assertIn(
                f"user backtrace of process {pid} (eu-stack):\n  PID {pid} - process\n"
                "    #1  0x00005619364c0976 <std[68c81ae915296b05]::thread::thread::Thread>::park\n",
                report,
            )
        self.assertNotIn("not complete", report)

    def test_a_detached_watched_process_is_in_the_report_but_not_stopped(self):
        # A copy of the server below target/ that makes its own session and
        # has no parent in this test (setsid -f forks, and its parent ends),
        # as a review host does.
        program = self.directory / "target" / "release" / SERVER
        program.parent.mkdir()
        shutil.copy2(shutil.which("sleep"), program)
        subprocess.run(["setsid", "-f", str(program), "300"], check=True)
        wait_for(lambda: pids_of(program), 10, "the detached server")
        (host,) = pids_of(program)
        self.addCleanup(os.kill, host, signal.SIGKILL)
        self.assertNotEqual(Path(f"/proc/{host}/stat").read_text().rsplit(") ", 1)[1].split()[1], str(os.getpid()))

        watcher = self.watcher_here()
        wait_for(lambda: report_module.Process(host).age(report_module.uptime()) > LIMIT + 1, 10, "the age of the host")
        watcher.check()
        # Its age alone does not start a report.
        self.assertEqual(watcher.out.getvalue(), "")

        tree, holder, waiter, server = self.started_tree()
        report = self.check_until_report(watcher)
        self.assertEqual(tree.wait(timeout=10), -signal.SIGKILL)
        self.assertEqual(report.count("::group::"), 1, report)
        self.assertIn("::group::Stuck test: stuck_case waits_for_the_lock", report)
        self.assertIn(
            f"Watched processes below {os.path.realpath(self.directory)}/target/ that are not below this test: "
            "1, with 1 processes in their trees. The watcher does not stop them.\n"
            f"process {host} (not below this test), parent ",
            report,
        )
        self.assertIn(f"command line: {program} 300\n", report)
        self.assertIn("The watcher killed the tree of 4 processes.\n", report)
        self.assertFalse(gone(host))

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
        self.assertRegex(
            report,
            r"\nThe watcher stops\. It looked at the processes \d+ times\. "
            rf"The largest age of a test binary that it saw was \d+ s\.\nThe watcher was alive at the stop\.\n$",
        )
        self.assertNotIn("::warning", report)
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

    def test_finish_with_no_start_gives_a_warning(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "finish"], env=self.environment, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0)
        self.assertEqual(
            result.stdout,
            "::warning title=Stuck test report::The stuck test report did not start, "
            "so no test of this step was watched.\n",
        )

    def test_finish_after_the_watcher_died_gives_a_warning(self):
        # The state that start leaves, with the id and start time of a
        # watcher that ended before the stop.
        watcher = self.watcher(LIMIT)
        first = watcher.stdout.readline()
        start = report_module.Process(watcher.pid).start
        log, pid_file = self.directory / "stuck-test-report.log", self.directory / "stuck-test-report.pid"
        pid_file.write_text(f"{watcher.pid} {start}")
        watcher.kill()
        log.write_text(first + watcher.communicate(timeout=10)[0])
        report = self.script("finish")
        self.assertIn("Watching the processes below", report)
        self.assertNotIn("The watcher stops.", report)
        self.assertNotIn("No test ran past the limit.", report)
        self.assertTrue(
            report.endswith(
                "::warning title=Stuck test report::The watcher was not alive at the stop. A test of this step "
                "that ran past the limit after the watcher ended has no report. The log of the watcher is above.\n"
            ),
            report,
        )
        self.assertFalse(pid_file.exists())

    def test_a_quiet_watcher_says_how_much_it_saw(self):
        tree = self.start_tree()
        self.script(
            "start", "--limit", "300", "--watch", SERVER, "--root", str(self.directory), "--interval", str(INTERVAL)
        )
        try:
            time.sleep(2)
        finally:
            report = self.script("finish")
        self.assertIsNone(tree.poll())
        match = re.search(r"It looked at the processes (\d+) times\. The largest age of a test binary that it saw was (\d+) s\.", report)
        self.assertIsNotNone(match, report)
        self.assertGreater(int(match[1]), 2)
        self.assertIn(int(match[2]), (1, 2, 3))
        self.assertTrue(report.endswith("The watcher was alive at the stop.\nNo test ran past the limit.\n"), report)

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
