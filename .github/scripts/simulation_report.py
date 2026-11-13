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
- Each other test whose last status is not a pass or a skip opens its own
  issue. A status word that the script does not know is a failure.

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
# The status line of a test: the status, right-aligned to 12 characters, the
# time in brackets, the counter and the name. The status words of nextest 0.9.143 are in
# nextest-runner/src/reporter/displayer/imp.rs, in the functions
# `write_status_line_prefix`, `status_str` and `short_status_str`.
TEST_LINE = re.compile(
    r"^ *([A-Z][A-Za-z0-9 +/-]*?) \[(>? *[0-9.]+s| *)\] +(?:\([^)\n]*\) +)?(\S+) (\S+)$", re.M
)
# The last status of a test that passed. FLAKY is a pass after a retry.
PASSED = {"PASS", "SLOW", "SLOW + LEAK", "SLOW+TMPASS", "LEAK", "TIMEOUT-PASS"}
PASSED_AFTER_RETRY = re.compile(r"FLAKY [0-9]+/[0-9]+")
SKIPPED = "SKIP"
# A status line while a test runs, before its last status: the time of a slow
# or a stopped test starts with ">"; a start has no time.
PROGRESS = re.compile(r"START|TRY [0-9]+ START")
# The last status of a test that nextest stopped at its time limit; TMT is the
# short form after a retry.
TIME_LIMIT = re.compile(r"TIMEOUT|TRY [0-9]+ TMT")
# Each status line and the summary line of nextest start a new part of the log.
STATUS_LINE = re.compile(r"^ *[A-Z][A-Za-z0-9 +/-]* \[(?:>? *[0-9.]+s| *)\]", re.M)
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
    (`<package>::<test target>`)."""
    package, _, target = binary_id.partition("::")
    command = (
        f"cargo nextest run --profile simulation --run-ignored all -p {package} "
        f"--test {target} -E 'test(={test})'"
    )
    return f"{environment} {command}" if environment else command


def is_progress(status, time):
    """Tells if a status line shows a test that still runs."""
    return time.startswith(">") or PROGRESS.fullmatch(status) is not None


def is_failure(status):
    """Tells if the last status of a test is not a pass or a skip."""
    passed = status in PASSED or PASSED_AFTER_RETRY.fullmatch(status)
    return not passed and status != SKIPPED


def failed_tests(log):
    """(status, binary id, test, output) of each test whose last status
    before the summary of nextest is not a pass or a skip, in the order of
    that status in the log. nextest prints the status line of a failed test
    again in its summary; the first output of the last status is kept."""
    summary = SUMMARY_LINE.search(log)
    run_end = summary.start() if summary else len(log)
    last = {}
    lines = {}
    for match in TEST_LINE.finditer(log):
        status, time, binary_id, test = match.groups()
        if is_progress(status, time):
            continue
        key = (binary_id, test)
        end = STATUS_LINE.search(log, match.end())
        output = log[match.end():end.start() if end else len(log)]
        lines.setdefault(key, []).append((status, output))
        if match.start() < run_end:
            last.pop(key, None)
            last[key] = status
    found = []
    for (binary_id, test), status in last.items():
        if not is_failure(status):
            continue
        outputs = [output for line, output in lines[(binary_id, test)] if line == status]
        output = next((output for output in outputs if output.strip()), outputs[0])
        found.append((status, binary_id, test, output))
    return found


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


def ended_with(status, output):
    """The end of a test with no panic: the status, and the exit code that
    nextest prints on Windows on the line after the status line."""
    first = output.lstrip("\n").splitlines()[:1]
    code = first[0].strip() if first and first[0].strip().startswith("- with code") else ""
    return f"the test ended with {status} {code}".strip()


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


def describe_run(layer, finished, run_url=None):
    """The report of a job that did not end its tests or gave no report;
    `finished` tells if the log shows the summary of nextest."""
    if finished:
        cause = (
            "The log shows the end of the tests, but no test in it gives a "
            "report. Read the log."
        )
    else:
        cause = (
            "The log does not show the end of the tests. The build failed, or "
            "the job stopped at its time limit. Read the log."
        )
    lines = [f"### {title(layer, NO_LAW)}", "", f"The job did not succeed. {cause}"]
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
    if kind == "time limit":
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
        elif TIME_LIMIT.fullmatch(status):
            issue_title = test_title(layer, test, "time limit")
            detail = "nextest stopped the test at its time limit"
        else:
            issue_title = test_title(layer, test, status)
            detail = panic_message(output) or ended_with(status, output)
        found.append((issue_title, describe_test(issue_title, test, detail, command, run_url)))
    if broken:
        found.append((title(layer, CHECK_BROKEN), describe_check_broken(layer, broken, run_url)))
    finished = SUMMARY_LINE.search(log) is not None
    if not found or not finished:
        found.append((title(layer, NO_LAW), describe_run(layer, finished, run_url)))
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
        found.append((title(NO_LAYER, NO_LAW), describe_run(NO_LAYER, False, run_url)))
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
