#!/usr/bin/env python3
"""Report the broken laws of a simulation run.

`summary` writes Markdown for the step summary of one layer. `issues` opens
one issue per report with the label `simulation`, or comments on the open
issue with the same title. A failure gives the law, the seed, the shrunk
steps, the replay line and the `#[hegel::reproduce_failure]` blob.

The log of a layer is the output of nextest. nextest prints a status line for
each test that did not pass, for example
`FAIL [   6.102s] (7/7) provenance-simulation::contract random_cases_...`,
followed by the output of the test. The status line names the package, the
test target and the test, so each report takes its replay line from it. Each
test that did not pass gives a report:

- A random case that broke a law writes a failure file, and the law of the
  last "simulation law ... broken" line of the log opens an issue. A shrink
  runs many failing cases, so one run can leave files of laws that the shrink
  passed through.
- A probe that broke a law prints "simulation probe broke law ..."; each probe
  and law opens its own issue.
- A planted test whose law did not see its fault prints "simulation check
  broken"; its layer gets an issue that says that a check is broken.
- Each other test that failed, and each test that nextest stopped at its time
  limit, opens its own issue.

When the log does not end with the summary of nextest (the build failed or
the job stopped at its time limit), or when no test gave a report, the layer
gets an issue that says that it failed without a broken law. Each report is
one more issue of the layer, so no report replaces another one.

The `summary` command runs in the job of the layer. It writes the variables
of the replay (`HEGEL_SEED`, `HEGEL_TEST_CASES` and `SIMULATION_L6`) of that
job to `<layer>.env` beside the log, and `issues` reads them from there.
"""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

BLOB = re.compile(r'#\[hegel::reproduce_failure\("([^"]*)"\)\]')
LAW_LINE = re.compile(r"simulation law (\S+) broken in layer (\S+):")
PROBE_LINE = re.compile(r"simulation probe broke law (\S+) in layer \S+: ([^\r\n]*)")
CHECK_BROKEN = "simulation check broken"
# The color codes that nextest writes in the log.
COLOR = re.compile(r"\x1b\[[0-9;]*m")
# The status line of a test that did not pass. A crash shows the signal on
# Linux and macOS (SIGABRT) and ABORT on Windows.
FAILED_TEST = re.compile(
    r"^ +(FAIL|TIMEOUT|ABORT|SIG[A-Z]+) \[[^\]]*\] (?:\([^)]*\) )?(\S+) (\S+)$", re.M
)
# Each status line and the summary line of nextest start a new part of the log.
STATUS_LINE = re.compile(r"^ +[A-Z][A-Za-z0-9 -]* \[ *>?[0-9.]+s\]", re.M)
SUMMARY_LINE = re.compile(r"^ +Summary \[", re.M)
PANIC_LINE = re.compile(r"panicked at [^\n]*:\n")
# The variables of the job that a replay needs.
REPLAY_VARIABLES = ("HEGEL_SEED", "HEGEL_TEST_CASES", "SIMULATION_L6")
GUIDE = (
    "https://github.com/quality-sh/provenance/blob/main/"
    "crates/provenance-simulation/README.md"
)
LABEL = "simulation"
NO_LAW = "run"
# A job that failed before it wrote a log names no layer.
NO_LAYER = "simulation"


def replay_environment(environ):
    """The variables of the replay that are set in `environ`."""
    return " ".join(f"{name}={environ[name]}" for name in REPLAY_VARIABLES if environ.get(name))


def replay(binary_id, test, environment):
    """The command that runs `test` of the nextest binary `binary_id`
    (`<package>::<test target>`, or `<package>` for the library)."""
    package, _, target = binary_id.partition("::")
    selection = f"--test {target}" if target else "--lib"
    command = (
        f"cargo nextest run --profile simulation --run-ignored all -p {package} "
        f"{selection} -E 'test(={test})'"
    )
    return f"{environment} {command}" if environment else command


def failed_tests(log):
    """(status, binary id, test, output) of each test that did not pass, in
    the order of the log. nextest prints the status line again in its
    summary, with no output; the first output is kept."""
    found = {}
    for match in FAILED_TEST.finditer(log):
        status, binary_id, test = match.groups()
        end = STATUS_LINE.search(log, match.end())
        output = log[match.end():end.start() if end else len(log)]
        key = (binary_id, test)
        if key not in found or not found[key][3].strip():
            found[key] = (status, binary_id, test, output)
    return list(found.values())


def panic_message(output):
    """The message of the last panic in `output`, on one line."""
    starts = list(PANIC_LINE.finditer(output))
    if not starts:
        return None
    lines = []
    for line in output[starts[-1].end():].splitlines():
        line = line.strip()
        if not line or line.startswith("note: "):
            break
        lines.append(line)
    return " ".join(lines)


def load_failures(directory):
    """The failure files of one layer, in name order."""
    path = Path(directory)
    if not path.is_dir():
        return []
    return [json.loads(file.read_text()) for file in sorted(path.glob("*.json"))]


def blob_for(log, failure):
    """The replay blob that Hegel printed with the report of `failure`.

    This is the first blob after the last report of the law. If Hegel printed
    the blob before the report, it is the nearest blob before it.
    """
    marker = f"simulation law {failure['law']} broken in layer {failure['layer']}"
    start = log.rfind(marker)
    if start < 0:
        return None
    after = BLOB.search(log, start)
    if after:
        return after.group(1)
    before = BLOB.findall(log, 0, start)
    return before[-1] if before else None


def title(layer, law):
    if law == NO_LAW:
        return f"Simulation: the {layer} layer failed without a broken law"
    if law == CHECK_BROKEN:
        return f"Simulation: a planted fault was not found in the {layer} layer"
    return f"Simulation: law {law} broken in the {layer} layer"


def last_law(log, layer):
    """The law of the last broken-law line of `layer` in the log, or None."""
    laws = [match.group(1) for match in LAW_LINE.finditer(log) if match.group(2) == layer]
    return laws[-1] if laws else None


def describe(failure, blob, replay_line, run_url=None):
    """The Markdown report of one failure."""
    lines = [
        f"### {title(failure['layer'], failure['law'])}",
        "",
        f"- Detail: {failure['detail']}",
        f"- Seed: `{failure.get('seed') or 'none'}`",
    ]
    if replay_line:
        lines.append(f"- Replay: `{replay_line}`")
    else:
        lines.append("- Replay: not printed; the test did not end with its report")
    if blob:
        lines.append(f'- Blob: `#[hegel::reproduce_failure("{blob}")]`')
    else:
        lines.append("- Blob: not printed; the case was not reproduced")
    if run_url:
        lines.append(f"- Run: {run_url}")
    lines.append(f"- What to do: {GUIDE}")
    lines += ["", "Shrunk steps:", "", "```json", json.dumps(failure["case"], indent=2), "```", ""]
    return "\n".join(lines)


def describe_run(layer, run_url=None):
    """The report of a job that did not end its tests or gave no report."""
    lines = [
        f"### {title(layer, NO_LAW)}",
        "",
        "The job did not succeed, and the log does not show the end of the "
        "tests or a report of a test. The build failed, or the job stopped at "
        "its time limit. Read the log.",
    ]
    if run_url:
        lines.append(f"Run: {run_url}")
    lines.append(f"What to do: {GUIDE}")
    return "\n".join(lines) + "\n"


def describe_check_broken(layer, broken, run_url=None):
    """The report of the planted tests of `layer` whose law did not see the
    planted fault; `broken` gives (test, line, replay) of each one."""
    lines = [
        f"### {title(layer, CHECK_BROKEN)}",
        "",
        "A planted test failed: its law did not see the planted fault, so the "
        "check itself is broken. A law issue of this run for the same layer "
        "is reported apart from this issue. The planted tests:",
        "",
    ]
    lines += [f"- `{test}`: `{line}`; replay: `{command}`" for test, line, command in broken]
    lines.append("")
    if run_url:
        lines.append(f"Run: {run_url}")
    lines.append(f"What to do: {GUIDE}")
    return "\n".join(lines) + "\n"


def test_title(layer, test, kind, law=None):
    """The title of the report of one test: a probe that broke `law`, a test
    that failed, or a test that did not end in its time limit."""
    if kind == "probe":
        return f"Simulation: probe {test} broke law {law} in the {layer} layer"
    if kind == "TIMEOUT":
        return f"Simulation: test {test} did not end in its time limit in the {layer} layer"
    return f"Simulation: test {test} failed in the {layer} layer"


def describe_test(issue_title, test, detail, replay_line, run_url=None):
    lines = [
        f"### {issue_title}",
        "",
        f"- Test: `{test}`",
        f"- Detail: {detail}",
        f"- Replay: `{replay_line}`",
    ]
    if run_url:
        lines.append(f"- Run: {run_url}")
    lines.append(f"- What to do: {GUIDE}")
    return "\n".join(lines) + "\n"


def layer_failures(log, layer, failures):
    """The failures of `layer` that open an issue: the law of the last
    broken-law line of the log, or each failure when the log has no such
    line."""
    failures = [failure for failure in failures if failure["layer"] == layer]
    law = last_law(log, layer)
    if law is None:
        return failures
    return [failure for failure in failures if failure["law"] == law]


def laws_in(output, layer):
    """The laws of the broken-law lines of `layer` in `output`."""
    return {match.group(1) for match in LAW_LINE.finditer(output) if match.group(2) == layer}


def layer_reports(layer, log, failures, run_url=None, environment=""):
    """(title, body) of each report of one layer that did not succeed."""
    log = COLOR.sub("", log)
    tests = failed_tests(log)
    found = []
    reported_laws = set()
    for failure in layer_failures(log, layer, failures):
        law = failure["law"]
        command = None
        for _, binary_id, test, output in tests:
            if law in laws_in(output, layer):
                command = replay(binary_id, test, environment)
        body = describe(failure, blob_for(log, failure), command, run_url)
        found.append((title(layer, law), body))
        reported_laws.add(law)
    broken = []
    for status, binary_id, test, output in tests:
        command = replay(binary_id, test, environment)
        probe = PROBE_LINE.search(output)
        if probe:
            law, detail = probe.groups()
            issue_title = test_title(layer, test, "probe", law)
        elif CHECK_BROKEN in output:
            line = next(line for line in output.splitlines() if CHECK_BROKEN in line)
            broken.append((test, line.strip(), command))
            continue
        elif laws_in(output, layer) & reported_laws:
            continue
        elif status == "TIMEOUT":
            issue_title = test_title(layer, test, status)
            detail = "nextest stopped the test at its time limit"
        else:
            issue_title = test_title(layer, test, status)
            detail = panic_message(output) or f"the test ended with {status}"
        found.append((issue_title, describe_test(issue_title, test, detail, command, run_url)))
    if broken:
        found.append((title(layer, CHECK_BROKEN), describe_check_broken(layer, broken, run_url)))
    if not found or not SUMMARY_LINE.search(log):
        found.append((title(layer, NO_LAW), describe_run(layer, run_url)))
    return found


def counts(stats_dir, layer):
    """The counts of the random cases of `layer` as a Markdown table."""
    path = Path(stats_dir) / f"{layer}.json" if stats_dir else None
    if path is None or not path.is_file():
        return ""
    rows = json.loads(path.read_text())
    lines = [f"### Counts of the {layer} layer", "", "| Count | Value |", "| --- | --- |"]
    lines += [f"| {name} | {value} |" for name, value in rows.items()]
    return "\n".join(lines) + "\n\n"


def summary(layer, log_path, failures_dir, stats_dir=None, succeeded=False, environ=os.environ):
    """The Markdown summary of one layer. When the layer did not succeed,
    also writes the variables of its replay to `<layer>.env` beside the
    log."""
    text = counts(stats_dir, layer)
    if succeeded:
        return text or f"The {layer} layer passed.\n"
    environment = replay_environment(environ)
    log_path = Path(log_path)
    if log_path.parent.is_dir():
        log_path.with_suffix(".env").write_text(environment + "\n")
    log = log_path.read_text(errors="replace") if log_path.is_file() else ""
    found = layer_reports(layer, log, load_failures(failures_dir), environment=environment)
    return text + "\n".join(body for _, body in found)


def reports(artifacts, run_url):
    """(title, body) for each report of each layer.

    Each job uploads `<layer>.log`, `<layer>.env` and its `failures`
    directory beside it. A download of one artifact unpacks it in
    `artifacts`; a download of more puts each in its own directory. So a
    layer is found from its log.
    """
    found = []
    for log_path in sorted(Path(artifacts).rglob("*.log")):
        layer = log_path.stem
        log = log_path.read_text(errors="replace")
        failures = load_failures(log_path.parent / "failures")
        env_path = log_path.with_suffix(".env")
        environment = env_path.read_text().strip() if env_path.is_file() else ""
        found += layer_reports(layer, log, failures, run_url, environment)
    if not found:
        found.append((title(NO_LAYER, NO_LAW), describe_run(NO_LAYER, run_url)))
    return found


def gh(*args):
    return subprocess.run(["gh", *args], check=True, text=True, capture_output=True).stdout


def open_issues(artifacts, run_url, run=gh, print_only=False, out=sys.stdout):
    """Opens or comments on one issue for each report. With `print_only`,
    makes no GitHub write: it reads the open issues and prints each issue
    that it would open and each comment that it would add."""
    if not print_only:
        run("label", "create", LABEL, "--force", "--color", "B60205",
            "--description", "A law of the nightly simulation broke")
    for issue_title, body in reports(artifacts, run_url):
        listed = json.loads(run("issue", "list", "--label", LABEL, "--state", "open",
                                "--search", f'"{issue_title}" in:title',
                                "--json", "number,title"))
        same = [issue for issue in listed if issue["title"] == issue_title]
        if print_only:
            action = (f"Would comment on issue #{same[0]['number']}" if same
                      else "Would open an issue")
            out.write(f"## {action}: {issue_title}\n\n{body}\n")
        elif same:
            run("issue", "comment", str(same[0]["number"]), "--body", body)
            out.write(f"Commented on issue #{same[0]['number']}: {issue_title}\n")
        else:
            run("issue", "create", "--title", issue_title, "--label", LABEL, "--body", body)
            out.write(f"Opened an issue: {issue_title}\n")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    one = commands.add_parser("summary")
    one.add_argument("--layer", required=True)
    one.add_argument("--log", required=True)
    one.add_argument("--failures", required=True)
    one.add_argument("--stats")
    one.add_argument("--job-status", default="failure")
    every = commands.add_parser("issues")
    every.add_argument("--artifacts", required=True)
    every.add_argument("--run-url", required=True)
    every.add_argument("--mode", choices=["issues", "print"], default="issues",
                       help="print: print each issue and comment, and make no GitHub write")
    args = parser.parse_args(argv)
    if args.command == "summary":
        sys.stdout.write(
            summary(args.layer, args.log, args.failures, args.stats, args.job_status == "success")
        )
    else:
        open_issues(args.artifacts, args.run_url, print_only=args.mode == "print")
    return 0


if __name__ == "__main__":
    sys.exit(main())
