#!/usr/bin/env python3
"""Reports and stops a test process that runs past a time limit (Linux only).

A CI job gives the limit and the names of the processes to watch:

    stuck_test_report.py start --limit 900 --watch provenance --root "$GITHUB_WORKSPACE"
    ... the test step ...
    stuck_test_report.py finish

`start` starts the watcher in the background and returns. The watcher runs as
root when `sudo -n` is available, because the kernel gives the system call
and the kernel stack of a thread, and eu-stack can attach to a process,
only to the parent of that process or to root. `finish` stops the watcher
and writes its log to the job log.

At each interval the watcher selects from `/proc` only the processes of the
user of `start` whose program file is below the `target/` directory of the
workspace root (`--root`). A process with a different program file (for
example a `provenance` that is installed in a different place) is reported
and stopped only as a member of the tree of a stuck test binary. Its name
alone, also when it is a watched name (`--watch`), does not select it.

Only a test binary (a program in `target/<profile>/deps/`, or in
`target/<profile>/build/<package>/<hash>/out/` in the new layout of Cargo)
with no test binary above it starts a report, when it is older than the limit. A watched
process never starts a report by its age: a server that a test starts lives
as long as its test, and a review host can make its own session and have the
init process as its parent.

The report is one log group: for each process of the tree of the test
binary, its parent, age, state and command line; for each thread, its state,
kernel wait channel, system call, kernel stack and user backtrace; the open
files and sockets; and the lines of `/proc/locks` on those files with the
holder and the waiter of each lock. Then the same facts for each watched
process below `target/` that is not in that tree, with its own tree, marked
as not below this test: a test can wait for a lock that such a process holds.
The install of the backtrace tool and the backtraces have one deadline for
the report (`REPORT_DEADLINE`); past it the report says that the backtraces
are not complete. An `::error` line names the test binary and its age, also
when the report has a fault. The watcher then kills the tree of the test
binary only, so the test runner reports the test as failed and runs the
other tests. The cleanup of the runner stops the other processes at the end
of the job. The watcher reports each test binary one time.
"""

import argparse
import functools
import os
import platform
import secrets
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import traceback
from pathlib import Path

CLOCK_TICKS = os.sysconf("SC_CLK_TCK") if hasattr(os, "sysconf") else 100
LINES_PER_BACKTRACE = 400
# The first words of the last line of the watcher, which it writes at SIGTERM.
STOP_LINE = "The watcher stops."
# The first character of each line of the log of the watcher: a workflow
# command of the watcher, or report text, which can hold any value.
COMMAND = "!"
TEXT = " "
# One deadline in seconds for the install of the backtrace tool and all the
# backtraces of one report. In the real CI run, the install took 9.1 s, and
# the install and the backtraces of 2 processes and 8 threads took less than
# 12 s. Past the deadline the report says that the backtraces are not
# complete, and the watcher stops the tree.
REPORT_DEADLINE = 60
# How long `finish` waits for a report in progress: the deadline, the reads
# of /proc and the stop.
FINISH_LIMIT = REPORT_DEADLINE + 60

# The x86_64 numbers of the system calls in which a test process can wait.
SYSTEM_CALLS = {
    0: "read", 1: "write", 3: "close", 7: "poll", 17: "pread64", 18: "pwrite64",
    23: "select", 24: "sched_yield", 34: "pause", 35: "nanosleep", 42: "connect",
    43: "accept", 44: "sendto", 45: "recvfrom", 46: "sendmsg", 47: "recvmsg",
    56: "clone", 59: "execve", 61: "wait4", 72: "fcntl", 73: "flock", 74: "fsync",
    75: "fdatasync", 128: "rt_sigtimedwait", 130: "rt_sigsuspend", 202: "futex",
    230: "clock_nanosleep", 232: "epoll_wait", 247: "waitid", 257: "openat",
    270: "pselect6", 271: "ppoll", 281: "epoll_pwait", 288: "accept4",
    435: "clone3", 441: "epoll_pwait2",
}

TCP_STATES = {
    "01": "ESTABLISHED", "02": "SYN_SENT", "03": "SYN_RECV", "04": "FIN_WAIT1",
    "05": "FIN_WAIT2", "06": "TIME_WAIT", "07": "CLOSE", "08": "CLOSE_WAIT",
    "09": "LAST_ACK", "0A": "LISTEN", "0B": "CLOSING",
}


def read(path):
    try:
        with open(path, "rb") as file:
            return file.read().decode("utf-8", "replace").strip()
    except OSError as error:
        return f"({error.strerror or error})"


def uptime():
    return float(read("/proc/uptime").split()[0])


class Process:
    """One process, read from `/proc/<pid>`."""

    def __init__(self, pid):
        self.pid = pid
        stat = Path(f"/proc/{pid}/stat").read_text()
        self.comm = stat[stat.index("(") + 1 : stat.rindex(")")]
        fields = stat[stat.rindex(")") + 2 :].split()
        self.state = fields[0]
        self.ppid = int(fields[1])
        self.start = int(fields[19])
        self.uid = os.stat(f"/proc/{pid}").st_uid
        try:
            self.exe = os.readlink(f"/proc/{pid}/exe").removesuffix(" (deleted)")
        except OSError:
            self.exe = ""

    @functools.cached_property
    def command(self):
        """The command line, read only for a process of a report."""
        return read(f"/proc/{self.pid}/cmdline").replace("\0", " ").strip()

    @property
    def name(self):
        return os.path.basename(self.exe) if self.exe else self.comm

    def age(self, clock):
        return clock - self.start / CLOCK_TICKS


def snapshot():
    processes = {}
    for entry in os.listdir("/proc"):
        if entry.isdigit():
            try:
                processes[int(entry)] = Process(int(entry))
            except (OSError, ValueError, IndexError):
                continue
    return processes


def tree(processes, top):
    """The pids of `top` and its descendants, each parent before its children."""
    children = {}
    for process in processes.values():
        children.setdefault(process.ppid, []).append(process.pid)
    order, pending = [], [(top, 0)]
    while pending:
        pid, depth = pending.pop()
        order.append((pid, depth))
        pending.extend((child, depth + 1) for child in sorted(children.get(pid, []), reverse=True))
    return order


def ancestors(processes, process):
    """The processes above `process`, from its parent to the highest one in `processes`."""
    seen = {process.pid}
    parent = processes.get(process.ppid)
    while parent and parent.pid not in seen:
        seen.add(parent.pid)
        yield parent
        parent = processes.get(parent.ppid)


def escape(text, title=False):
    text = text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    return text.replace(":", "%3A").replace(",", "%2C") if title else text


class Watcher:
    def __init__(self, limit, names, root, uid, install_tool, out):
        self.limit = limit
        self.names = set(names)
        self.target = os.path.join(os.path.realpath(root), "target") + os.sep
        self.uid = uid
        self.install_tool = install_tool
        self.out = out
        self.reported = set()
        self.tool = None
        self.deadline = 0.0
        self.checks = 0
        self.largest_age = 0.0

    def below_target(self, process):
        return process.uid == self.uid and process.exe.startswith(self.target)

    def test_binary(self, process):
        """A program where Cargo writes a test binary, and not a build script.

        Up to Rust 1.99 that is `target/<profile>/deps/<file>`. From Rust 1.100,
        with the new layout of the build directory, it is
        `target/<profile>/build/<package>/<hash>/out/<file>`.
        """
        if not self.below_target(process) or process.name.startswith("build-script-"):
            return False
        parts = process.exe[len(self.target):].split(os.sep)
        return parts[-2:-1] == ["deps"] or (len(parts) >= 5 and parts[-2] == "out" and parts[-5] == "build")

    def watched(self, process):
        return self.below_target(process) and process.name in self.names

    def tops(self, processes):
        """The test binaries with no test binary above them. Only these start a report."""
        return [
            process for process in processes.values()
            if self.test_binary(process) and not any(self.test_binary(above) for above in ancestors(processes, process))
        ]

    def others(self, processes, members):
        """The highest watched processes that are not in the tree of the stuck test."""
        outside = [
            process for process in processes.values()
            if self.watched(process) and process.pid not in members
        ]
        pids = {process.pid for process in outside}
        return [
            process for process in sorted(outside, key=lambda process: process.pid)
            if not any(above.pid in pids for above in ancestors(processes, process))
        ]

    def check(self):
        processes = snapshot()
        clock = uptime()
        self.checks += 1
        for top in self.tops(processes):
            self.largest_age = max(self.largest_age, top.age(clock))
            key = (top.pid, top.start)
            if top.age(clock) > self.limit and key not in self.reported:
                self.reported.add(key)
                self.report(processes, top, clock)

    def run(self, interval):
        """Checks at each interval until SIGTERM. A report in progress completes first."""
        stopping = []
        signal.signal(signal.SIGTERM, lambda *_: stopping.append(True))
        self.write(f"Watching the processes below {self.target} with a limit of {self.limit:g} s.")
        while not stopping:
            try:
                self.check()
            except Exception as error:  # A failed check must not stop the watch.
                self.write(f"The check failed: {error!r}")
            deadline = time.monotonic() + interval
            while not stopping and time.monotonic() < deadline:
                time.sleep(min(0.2, interval))
        self.write(
            f"{STOP_LINE} It looked at the processes {self.checks} times. "
            f"The largest age of a test binary that it saw was {self.largest_age:.0f} s."
        )

    def write(self, text):
        """Writes one line of report text. A newline or a carriage return in a value becomes visible."""
        self.out.write(TEXT + text.replace("\r", "\\r").replace("\n", "\\n") + "\n")
        self.out.flush()

    def command(self, text):
        """Writes one workflow command of the watcher. `finish` writes it outside the report text."""
        self.out.write(COMMAND + text.replace("\r", "%0D").replace("\n", "%0A") + "\n")
        self.out.flush()

    def report(self, processes, top, clock):
        label = test_label(top)
        age = top.age(clock)
        members = tree(processes, top.pid)
        self.deadline = time.monotonic() + REPORT_DEADLINE
        self.command(f"::group::Stuck test: {label} (process {top.pid}, {age:.0f} s)")
        try:
            self.write(f"The test process {top.pid} ran {age:.0f} s, past the limit of {self.limit:g} s.")
            self.write(f"Its tree has {len(members)} processes.")
            files = {}
            for pid, depth in members:
                process = processes.get(pid)
                if process:
                    self.describe(process, depth, clock, files)
            others = self.others(processes, {pid for pid, _ in members})
            other_members = [member for other in others for member in tree(processes, other.pid)]
            self.write(
                f"Watched processes below {self.target} that are not below this test: "
                + (f"{len(others)}, with {len(other_members)} processes in their trees." if others else "none.")
                + (" The watcher does not stop them." if others else "")
            )
            for pid, depth in other_members:
                process = processes.get(pid)
                if process:
                    self.describe(process, depth, clock, files, " (not below this test)")
            self.write_locks(files, processes)
            self.write_backtraces([pid for pid, _ in members + other_members])
        except Exception as error:  # The stop below must occur after any fault of the report.
            place = traceback.extract_tb(error.__traceback__)[-1]
            self.write(
                f"The report is not complete: {error!r} at line {place.lineno} ({place.name}). "
                "The watcher stops the tree."
            )
        finally:
            self.command("::endgroup::")
            # The error line comes before the stop: when the tree ends, the
            # test step ends, and the job can stop the watcher.
            self.command(
                f"::error title={escape('Stuck test ' + label, title=True)}::"
                + escape(
                    f"The test {label} ran {age:.0f} s, past the limit of {self.limit:g} s. "
                    f"The log group 'Stuck test: {label}' gives the report. The watcher stops its process tree."
                )
            )
            self.write(stop(top))

    def describe(self, process, depth, clock, files, mark=""):
        indent = "  " * depth
        self.write(
            f"{indent}process {process.pid}{mark}, parent {process.ppid}, age {process.age(clock):.0f} s, "
            f"state {process.state}, program {process.exe or process.comm}"
        )
        self.write(f"{indent}  command line: {process.command}")
        try:
            tasks = sorted(os.listdir(f"/proc/{process.pid}/task"), key=int)
        except OSError as error:
            self.write(f"{indent}  threads: ({error.strerror or error})")
            tasks = []
        for task in tasks:
            base = f"/proc/{process.pid}/task/{task}"
            stat = read(f"{base}/stat")
            state = stat[stat.rfind(")") + 2 :].split(" ", 1)[0] if ")" in stat else "?"
            self.write(
                f"{indent}  thread {task} {read(base + '/comm')}: state {state}, "
                f"wait channel {read(base + '/wchan') or '0'}, system call {system_call(read(base + '/syscall'))}"
            )
            stack = read(f"{base}/stack")
            if stack and not stack.startswith("("):
                frames = [line.split("] ", 1)[-1] for line in stack.splitlines()]
                self.write(f"{indent}    kernel stack: {' < '.join(frames)}")
        sockets = socket_table(process.pid)
        listening = []
        directory = f"/proc/{process.pid}/fd"
        try:
            descriptors = sorted(os.listdir(directory), key=int)
        except OSError as error:
            self.write(f"{indent}  open files: ({error.strerror})")
            descriptors = []
        for descriptor in descriptors:
            path = f"{directory}/{descriptor}"
            try:
                target = os.readlink(path)
            except OSError as error:
                target = f"({error.strerror})"
            detail = ""
            if target.startswith("socket:["):
                detail = sockets.get(target[8:-1], "")
                if " LISTEN" in detail:
                    listening.append(detail.split()[1])
            else:
                try:
                    status = os.stat(path)
                    device = f"{os.major(status.st_dev):02x}:{os.minor(status.st_dev):02x}"
                    files.setdefault(f"{device}:{status.st_ino}", target)
                except OSError:
                    pass
            self.write(f"{indent}  file {descriptor}: {target}{' ' + detail if detail else ''}")
        self.write(f"{indent}  listening ports: {', '.join(listening) if listening else 'none'}")

    def write_locks(self, files, processes):
        self.write("locks on the open files of the tree (/proc/locks):")
        lines = 0
        for line in read("/proc/locks").splitlines():
            fields = line.split()
            waits = len(fields) > 1 and fields[1] == "->"
            rest = fields[2:] if waits else fields[1:]
            if len(rest) < 5:
                continue
            if rest[4] not in files:
                continue
            pid = rest[3]
            holder = processes.get(int(pid)) if pid.isdigit() else None
            who = f"process {pid}: {holder.command}" if holder else f"process {pid}"
            verb = "waits for" if waits else "holds"
            self.write(f"  {who} {verb} a {rest[0]} {rest[2]} lock on {files[rest[4]]} ({line})")
            lines += 1
        if not lines:
            self.write("  none")

    def remaining(self):
        return self.deadline - time.monotonic()

    def backtrace_tool(self):
        if self.tool is None:
            self.tool = find_tool()
            if not self.tool:
                self.write("user backtraces: the runner has no eu-stack.")
                if self.install_tool:
                    self.tool = install_tool(self.write, self.remaining)
        return self.tool

    def write_backtraces(self, pids):
        tool = self.backtrace_tool()
        if not tool:
            if self.remaining() <= 0:
                self.write(f"The user backtraces are not complete: the report deadline of {REPORT_DEADLINE:g} s passed.")
            return
        for index, pid in enumerate(pids):
            if self.remaining() <= 0:
                self.write(
                    f"The user backtraces are not complete: the report deadline of {REPORT_DEADLINE:g} s passed "
                    f"before the backtrace of process {', '.join(str(left) for left in pids[index:])}."
                )
                return
            self.write(f"user backtrace of process {pid} (eu-stack):")
            code, output = run_limited([tool, "-p", str(pid)], self.remaining())
            lines = output.splitlines()
            if code is None:
                lines.append(f"(eu-stack did not end before the report deadline of {REPORT_DEADLINE:g} s)")
            for line in demangle(lines[:LINES_PER_BACKTRACE], self.remaining()):
                self.write(f"  {line}")
            if len(lines) > LINES_PER_BACKTRACE:
                self.write(f"  ({len(lines) - LINES_PER_BACKTRACE} more lines are not shown)")


def run_limited(command, seconds, environment=None, text_input=None):
    """Runs a command for at most `seconds`. Gives its exit code and its output.

    The exit code is None when the command did not end in time. The command
    runs in a new session, so that the stop at the limit also stops its
    children, and the read of its output ends.
    """
    if seconds <= 0:
        return None, ""
    try:
        child = subprocess.Popen(
            command, stdin=subprocess.PIPE if text_input is not None else subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=environment,
            text=True, errors="replace", start_new_session=True,
        )
    except OSError as error:
        return 127, f"({os.path.basename(command[0])} did not start: {error.strerror or error})"
    try:
        output = child.communicate(text_input, timeout=seconds)[0]
        return child.returncode, output
    except subprocess.TimeoutExpired:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except OSError:
            pass
        return None, child.communicate()[0]


def demangle(lines, seconds):
    """Rust symbol names in a readable form, when binutils gives c++filt."""
    if not lines or not shutil.which("c++filt"):
        return lines
    code, output = run_limited(["c++filt"], min(seconds, 10), text_input="\n".join(lines))
    demangled = output.splitlines()
    return demangled if code == 0 and len(demangled) == len(lines) else lines


def test_label(process):
    """The name of the test binary without its hash, and the test name if the command line gives it."""
    name = process.name
    stem, _, suffix = name.rpartition("-")
    if stem and len(suffix) == 16 and all(c in "0123456789abcdef" for c in suffix):
        name = stem
    arguments = [part for part in process.command.split()[1:] if not part.startswith("-")]
    return f"{name} {arguments[0]}" if arguments else name


def system_call(text, machine=platform.machine()):
    """The system call from `/proc/<pid>/task/<tid>/syscall`: the name and number on x86_64, else the number."""
    if text.startswith("("):
        return text
    first = text.split(" ", 1)[0]
    if first.lstrip("-").isdigit():
        number = int(first)
        if number < 0:
            return "none (blocked outside a system call)"
        if machine == "x86_64":
            return f"{SYSTEM_CALLS.get(number, 'number')} ({number})"
        return str(number)
    return first or "?"


def socket_table(pid):
    """Socket inode to a description, from the network tables of the process."""
    table = {}
    for kind in ("tcp", "tcp6", "udp", "udp6"):
        for line in read(f"/proc/{pid}/net/{kind}").splitlines()[1:]:
            fields = line.split()
            if len(fields) > 9:
                state = TCP_STATES.get(fields[3], fields[3]) if kind.startswith("tcp") else ""
                table[fields[9]] = f"{kind} {address(fields[1])} to {address(fields[2])} {state}".strip()
    for line in read(f"/proc/{pid}/net/unix").splitlines()[1:]:
        fields = line.split()
        if len(fields) > 6:
            path = fields[7] if len(fields) > 7 else "(no path)"
            table[fields[6]] = f"unix {path} state {fields[5]}"
    return table


def address(text):
    host, _, port = text.partition(":")
    if len(host) == 8:
        octets = bytes.fromhex(host)[::-1]
        host = ".".join(str(octet) for octet in octets)
    elif len(host) == 32:
        words = [bytes.fromhex(host[i : i + 8])[::-1].hex() for i in range(0, 32, 8)]
        joined = "".join(words)
        host = "[" + ":".join(joined[i : i + 4] for i in range(0, 32, 4)) + "]"
    return f"{host}:{int(port, 16) if port else '?'}"


def find_tool():
    """The path of eu-stack. The runner image has no backtrace tool, and the install gives eu-stack."""
    return shutil.which("eu-stack")


def install_tool(write, remaining):
    """Installs elfutils for eu-stack before the report deadline. Only a job with a stuck test pays for it."""
    began = time.monotonic()
    environment = {**os.environ, "DEBIAN_FRONTEND": "noninteractive"}
    install = ["apt-get", "install", "-y", "--no-install-recommends", "elfutils"]
    for commands in ([install], [["apt-get", "update"], install]):
        if all(run_limited(command, remaining(), environment)[0] == 0 for command in commands):
            write(f"Installed elfutils for eu-stack in {time.monotonic() - began:.1f} s.")
            return find_tool()
    write(f"The install of elfutils failed after {time.monotonic() - began:.1f} s.")
    return None


def send(process, number):
    """Sends a signal to `process` only if its id has the start time of the snapshot.

    A different start time means that the process ended and a different
    process got its id. A pidfd keeps the check and the signal on one process.
    """
    try:
        handle = os.pidfd_open(process.pid)
    except OSError:
        return False
    try:
        if Process(process.pid).start != process.start:
            return False
        signal.pidfd_send_signal(handle, number)
        return True
    except (OSError, ValueError, IndexError):
        return False
    finally:
        os.close(handle)


def stop(top):
    """Stops the tree of `top`: each process first, so that none starts a new child, then kills each one."""
    processes = snapshot()
    current = processes.get(top.pid)
    if current is None or current.start != top.start:
        return f"The test process {top.pid} ended before the stop. The watcher sent no signal."
    stopped = {}
    # A child that a process started before its stop is in the next snapshot.
    for _ in range(10):
        new = [processes[pid] for pid, _ in tree(processes, top.pid) if pid in processes and pid not in stopped]
        if not new:
            break
        for process in new:
            if send(process, signal.SIGSTOP):
                stopped[process.pid] = process
        processes = snapshot()
    for process in stopped.values():
        send(process, signal.SIGKILL)
    deadline = time.monotonic() + 10
    alive = sorted(stopped)
    while alive and time.monotonic() < deadline:
        alive = [pid for pid in alive if running(pid, stopped[pid].start)]
        time.sleep(0.1)
    if alive:
        return f"The watcher killed the tree of {len(stopped)} processes; {alive} did not end in 10 s."
    return f"The watcher killed the tree of {len(stopped)} processes."


def state_files():
    directory = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir())
    return directory / "stuck-test-report.log", directory / "stuck-test-report.pid"


def start(arguments):
    if not sys.platform.startswith("linux"):
        print("The stuck test report works only on Linux. No watcher started.")
        return 0
    log, pid_file = state_files()
    command = [
        sys.executable, os.path.abspath(__file__), "watch",
        "--limit", str(arguments.limit),
        "--root", os.path.abspath(arguments.root),
        "--uid", str(os.getuid()),
        "--interval", str(arguments.interval),
    ]
    for name in arguments.watch:
        command += ["--watch", name]
    if os.environ.get("GITHUB_ACTIONS") == "true":
        command.append("--install-tool")
    if os.geteuid() != 0 and shutil.which("sudo") and subprocess.run(["sudo", "-n", "true"], capture_output=True).returncode == 0:
        command = ["sudo", "-n", "--"] + command
    with open(log, "wb") as output:
        watcher = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
    # The start time identifies the process, also if a different process gets its id.
    pid_file.write_text(f"{watcher.pid} {Process(watcher.pid).start}")
    print(f"Started the stuck test report (process {watcher.pid}, limit {arguments.limit:g} s, log {log}).")
    return 0


def running(pid, start=None):
    """True when the process `pid` runs, is not a zombie and, if given, has the start time `start`."""
    try:
        process = Process(pid)
    except (OSError, ValueError, IndexError):
        return False
    return process.state != "Z" and (start is None or process.start == start)


def warning(text):
    print(f"::warning title=Stuck test report::{escape(text)}")


def render(lines):
    """The log for the job log: the commands of the watcher, and the report text between stop-commands lines.

    The runner does not run a workflow command in the text between
    `::stop-commands::<token>` and `::<token>::`. The token is random, so a
    value in the report text cannot end that block.
    """
    token = secrets.token_hex(16)
    out, inside = [], False
    for line in lines:
        if line.startswith(COMMAND):
            if inside:
                out.append(f"::{token}::")
                inside = False
            out.append(line[len(COMMAND):])
        else:
            if not inside:
                out.append(f"::stop-commands::{token}")
                inside = True
            # A line with no mark (for example a Python traceback) is text too.
            out.append(line[len(TEXT):] if line.startswith(TEXT) else line)
    if inside:
        out.append(f"::{token}::")
    return "".join(line + "\n" for line in out)


def finish(_arguments):
    """Stops the watcher and writes its log. Always gives exit code 0."""
    log, pid_file = state_files()
    if not pid_file.exists():
        warning("The stuck test report did not start, so no test of this step was watched.")
        return 0
    pid, start = (int(field) for field in pid_file.read_text().split())
    alive = running(pid, start)
    if alive:
        try:
            os.kill(pid, signal.SIGTERM)
        except PermissionError:
            subprocess.run(["sudo", "-n", "kill", "-TERM", str(pid)], check=False)
        except ProcessLookupError:
            pass
        # The watcher completes a report in progress before it ends.
        deadline = time.monotonic() + FINISH_LIMIT
        while time.monotonic() < deadline and running(pid, start):
            time.sleep(0.2)
    pid_file.unlink()
    lines = log.read_text(errors="replace").split("\n") if log.exists() else []
    if lines and not lines[-1]:
        lines.pop()
    sys.stdout.write(render(lines))
    if not alive:
        warning(
            "The watcher was not alive at the stop. A test of this step that ran past the limit "
            "after the watcher ended has no report. The log of the watcher is above."
        )
    elif not any(line.startswith(TEXT + STOP_LINE) for line in lines):
        warning(f"The watcher was alive at the stop, but it did not end in {FINISH_LIMIT:g} s.")
    else:
        print("The watcher was alive at the stop.")
        if not any(line.startswith(COMMAND + "::error ") for line in lines):
            print("No test ran past the limit.")
    return 0


def watch(arguments):
    watcher = Watcher(arguments.limit, arguments.watch, arguments.root, arguments.uid, arguments.install_tool, sys.stdout)
    watcher.run(arguments.interval)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("start", "watch"):
        command = commands.add_parser(name)
        command.add_argument("--limit", type=float, required=True, help="seconds that a test process can run")
        command.add_argument("--watch", action="append", default=[], help="a process name to watch")
        command.add_argument("--root", required=True, help="the workspace root; the test binaries are below its target/ directory")
        command.add_argument("--interval", type=float, default=15.0, help="seconds between two checks")
    commands.choices["watch"].add_argument("--uid", type=int, default=os.getuid())
    commands.choices["watch"].add_argument("--install-tool", action="store_true")
    commands.add_parser("finish")
    arguments = parser.parse_args(argv)
    return {"start": start, "watch": watch, "finish": finish}[arguments.command](arguments)


if __name__ == "__main__":
    sys.exit(main())
