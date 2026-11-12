#!/usr/bin/env python3
"""Reports and stops a test process that runs past a time limit (Linux only).

A CI job gives the limit and the names of the processes to watch:

    stuck_test_report.py start --limit 900 --watch provenance
    ... the test step ...
    stuck_test_report.py finish

`start` starts the watcher in the background and returns. The watcher runs as
root when `sudo -n` is available, because the kernel gives the system call
and the kernel stack of a thread, and a debugger can attach to a process,
only to the parent of that process or to root. `finish` stops the watcher
and writes its log to the job log.

At each interval the watcher reads the processes of the job from `/proc`.
A test binary is the top process of a tree whose program file is below the
`target/` directory. A watched process (for example `provenance`) that has
no test binary above it is also a top process. The limit applies to the top
process only: a server that a test starts lives as long as its test.

When a top process is older than the limit, the watcher writes one report in
a log group: for each process of the tree, its parent, age, state and command
line; for each thread, its state, kernel wait channel, system call, kernel
stack and user backtrace; the open files and sockets; and the lines of
`/proc/locks` on those files with the holder and the waiter of each lock.
An `::error` line names the test binary and its age. The watcher then kills
the tree, so the test runner reports the test as failed and runs the other
tests. The watcher reports each top process one time.
"""

import argparse
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

CLOCK_TICKS = os.sysconf("SC_CLK_TCK") if hasattr(os, "sysconf") else 100
BACKTRACE_LIMIT = 120
INSTALL_LIMIT = 240
LINES_PER_BACKTRACE = 400
# How long `finish` waits for a report in progress: the install and one
# backtrace of each process of a small tree.
FINISH_LIMIT = 600

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
        self.command = read(f"/proc/{pid}/cmdline").replace("\0", " ").strip()

    @property
    def name(self):
        return os.path.basename(self.exe) if self.exe else self.comm

    def age(self, now):
        return now - self.start / CLOCK_TICKS


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


def escape(text, title=False):
    text = text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    return text.replace(":", "%3A").replace(",", "%2C") if title else text


class Watcher:
    def __init__(self, limit, names, root, uid, install_tool, out):
        self.limit = limit
        self.names = set(names)
        self.root = os.path.realpath(root) + os.sep
        self.uid = uid
        self.install_tool = install_tool
        self.out = out
        self.reported = set()
        self.tool = None

    def below_root(self, process):
        return process.exe.startswith(self.root) and not process.name.startswith("build-script-")

    def tops(self, processes):
        """The test binaries and the watched processes with no test binary above them."""
        tops = []
        for process in processes.values():
            if process.uid != self.uid:
                continue
            if not (self.below_root(process) or process.name in self.names):
                continue
            parent = processes.get(process.ppid)
            while parent and not self.below_root(parent):
                parent = processes.get(parent.ppid)
            if parent is None:
                tops.append(process)
        return tops

    def check(self):
        processes = snapshot()
        now = uptime()
        for top in self.tops(processes):
            key = (top.pid, top.start)
            if top.age(now) > self.limit and key not in self.reported:
                self.reported.add(key)
                self.report(processes, top, now)

    def run(self, interval):
        """Checks at each interval until SIGTERM. A report in progress completes first."""
        stopping = []
        signal.signal(signal.SIGTERM, lambda *_: stopping.append(True))
        self.write(f"Watching the processes below {self.root} and {sorted(self.names)} with a limit of {self.limit:g} s.")
        while not stopping:
            try:
                self.check()
            except Exception as error:  # A failed check must not stop the watch.
                self.write(f"The check failed: {error!r}")
            deadline = time.monotonic() + interval
            while not stopping and time.monotonic() < deadline:
                time.sleep(min(0.2, interval))

    def write(self, text):
        self.out.write(text + "\n")
        self.out.flush()

    def report(self, processes, top, now):
        label = test_label(top)
        age = top.age(now)
        members = tree(processes, top.pid)
        self.write(f"::group::Stuck test: {label} (process {top.pid}, {age:.0f} s)")
        self.write(f"The test process {top.pid} ran {age:.0f} s, past the limit of {self.limit:g} s.")
        self.write(f"Its tree has {len(members)} processes.")
        files = {}
        for pid, depth in members:
            process = processes.get(pid)
            if process:
                self.describe(process, depth, now, files)
        self.write_locks(files, processes)
        self.write_backtraces([pid for pid, _ in members])
        self.write("::endgroup::")
        # The error line comes before the stop: when the tree ends, the test
        # step ends, and the job can stop the watcher soon after.
        self.write(
            f"::error title={escape('Stuck test ' + label, title=True)}::"
            + escape(
                f"The test {label} ran {age:.0f} s, past the limit of {self.limit:g} s. "
                f"The log group 'Stuck test: {label}' gives the report. The watcher stops its process tree."
            )
        )
        self.write(stop(pid for pid, _ in tree(snapshot(), top.pid)))

    def describe(self, process, depth, now, files):
        indent = "  " * depth
        self.write(
            f"{indent}process {process.pid}, parent {process.ppid}, age {process.age(now):.0f} s, "
            f"state {process.state}, program {process.exe or process.comm}"
        )
        self.write(f"{indent}  command line: {process.command}")
        for task in sorted(os.listdir(f"/proc/{process.pid}/task"), key=int) if os.path.isdir(f"/proc/{process.pid}/task") else []:
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

    def backtrace_tool(self):
        if self.tool is None:
            self.tool = find_tool()
            if not self.tool:
                self.write("user backtraces: the runner has no eu-stack, gdb or lldb.")
                if self.install_tool:
                    self.tool = install_tool(self.write)
        return self.tool

    def write_backtraces(self, pids):
        tool = self.backtrace_tool()
        if not tool:
            return
        for pid in pids:
            command = {
                "eu-stack": ["eu-stack", "-p", str(pid)],
                "gdb": ["gdb", "-p", str(pid), "-batch", "-nx", "-ex", "thread apply all bt"],
                "lldb": ["lldb", "-p", str(pid), "--batch", "-o", "thread backtrace all"],
            }[tool]
            self.write(f"user backtrace of process {pid} ({tool}):")
            try:
                result = subprocess.run(command, capture_output=True, text=True, errors="replace", timeout=BACKTRACE_LIMIT)
                lines = (result.stdout + result.stderr).splitlines()
            except subprocess.TimeoutExpired:
                lines = [f"({tool} did not end in {BACKTRACE_LIMIT} s)"]
            for line in demangle(lines[:LINES_PER_BACKTRACE]):
                self.write(f"  {line}")
            if len(lines) > LINES_PER_BACKTRACE:
                self.write(f"  ({len(lines) - LINES_PER_BACKTRACE} more lines are not shown)")


def demangle(lines):
    """Rust symbol names in a readable form, when binutils gives c++filt."""
    if not lines or not shutil.which("c++filt"):
        return lines
    try:
        result = subprocess.run(["c++filt"], input="\n".join(lines), capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return lines
    demangled = result.stdout.splitlines()
    return demangled if result.returncode == 0 and len(demangled) == len(lines) else lines


def test_label(process):
    """The name of the test binary without its hash, and the test name if the command line gives it."""
    name = process.name
    stem, _, suffix = name.rpartition("-")
    if stem and len(suffix) == 16 and all(c in "0123456789abcdef" for c in suffix):
        name = stem
    arguments = [part for part in process.command.split()[1:] if not part.startswith("-")]
    return f"{name} {arguments[0]}" if arguments else name


def system_call(text):
    if text.startswith("("):
        return text
    first = text.split(" ", 1)[0]
    if first.lstrip("-").isdigit():
        number = int(first)
        return f"{SYSTEM_CALLS.get(number, 'number')} ({number})" if number >= 0 else "none (blocked outside a system call)"
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
    for tool in ("eu-stack", "gdb", "lldb"):
        if shutil.which(tool):
            return tool
    return None


def install_tool(write):
    """Installs elfutils for eu-stack. Only a job with a stuck test pays for it."""
    began = time.monotonic()
    environment = {**os.environ, "DEBIAN_FRONTEND": "noninteractive"}
    install = ["apt-get", "install", "-y", "--no-install-recommends", "elfutils"]
    for commands in ([install], [["apt-get", "update"], install]):
        try:
            if all(
                subprocess.run(command, env=environment, capture_output=True, timeout=INSTALL_LIMIT).returncode == 0
                for command in commands
            ):
                write(f"Installed elfutils for eu-stack in {time.monotonic() - began:.1f} s.")
                return find_tool()
        except (OSError, subprocess.TimeoutExpired):
            pass
    write(f"The install of elfutils failed after {time.monotonic() - began:.1f} s.")
    return None


def stop(pids):
    """Stops each process first, so that none starts a new child, then kills each one."""
    pids = list(pids)
    for sent in (signal.SIGSTOP, signal.SIGKILL):
        for pid in pids:
            try:
                os.kill(pid, sent)
            except OSError:
                pass
    deadline = time.monotonic() + 10
    alive = pids
    while alive and time.monotonic() < deadline:
        alive = [pid for pid in alive if os.path.exists(f"/proc/{pid}") and read(f"/proc/{pid}/stat").split(") ")[-1][:1] != "Z"]
        time.sleep(0.1)
    if alive:
        return f"The watcher killed the tree of {len(pids)} processes; {alive} did not end in 10 s."
    return f"The watcher killed the tree of {len(pids)} processes."


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
    pid_file.write_text(str(watcher.pid))
    print(f"Started the stuck test report (process {watcher.pid}, limit {arguments.limit:g} s, log {log}).")
    return 0


def running(pid):
    stat = read(f"/proc/{pid}/stat")
    return not stat.startswith("(") and stat.rsplit(") ", 1)[-1][:1] != "Z"


def finish(_arguments):
    log, pid_file = state_files()
    if pid_file.exists():
        pid = int(pid_file.read_text())
        try:
            os.kill(pid, signal.SIGTERM)
        except PermissionError:
            subprocess.run(["sudo", "-n", "kill", "-TERM", str(pid)], check=False)
        except ProcessLookupError:
            pass
        # The watcher completes a report in progress before it ends.
        deadline = time.monotonic() + FINISH_LIMIT
        while time.monotonic() < deadline and running(pid):
            time.sleep(0.2)
        pid_file.unlink()
    if not log.exists():
        print("The stuck test report did not run.")
        return 0
    text = log.read_text(errors="replace")
    sys.stdout.write(text)
    if "::error" not in text:
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
        command.add_argument("--root", default="target", help="the directory of the test binaries")
        command.add_argument("--interval", type=float, default=15.0, help="seconds between two checks")
    commands.choices["watch"].add_argument("--uid", type=int, default=os.getuid())
    commands.choices["watch"].add_argument("--install-tool", action="store_true")
    commands.add_parser("finish")
    arguments = parser.parse_args(argv)
    return {"start": start, "watch": watch, "finish": finish}[arguments.command](arguments)


if __name__ == "__main__":
    sys.exit(main())
