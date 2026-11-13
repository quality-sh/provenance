#!/usr/bin/env python3
"""Report the broken laws of a simulation run.

`summary` writes Markdown for the step summary of one layer. `issues` opens
one issue per layer and law with the label `simulation`, or comments on the
open issue with the same title. A failure gives the law, the seed, the shrunk
steps, the replay line and the `#[hegel::reproduce_failure]` blob.

A shrink runs many failing cases, and each one writes the failure file of its
law, so one run can leave files of laws that the shrink passed through. Only
the law of the last "simulation law ... broken" line of a layer's log opens an
issue. A probe that breaks a law prints "simulation probe <test> broke law
..." and the command that runs the probe; each such probe and law opens its
own issue. A probe whose other check failed prints "simulation probe <test>
failed a check ..." and the same command; each such probe opens its own
issue. A planted test whose law did not see its fault prints "simulation
check broken"; its layer gets an issue that says that a check is broken, in
addition to the law issues of the layer.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

BLOB = re.compile(r'#\[hegel::reproduce_failure\("([^"]*)"\)\]')
LAW_LINE = re.compile(r"simulation law (\S+) broken in layer (\S+):")
PROBE_LINE = re.compile(
    r"simulation probe (\S+) (?:broke law (\S+)|failed a check) in layer (\S+): "
    r"([^\r\n]*)\r?\nreplay: ([^\r\n]*)"
)
CHECK_BROKEN = "simulation check broken"
GUIDE = (
    "https://github.com/quality-sh/provenance/blob/main/"
    "crates/provenance-simulation/README.md"
)
LABEL = "simulation"
NO_LAW = "run"
# A job that failed before it wrote a log names no layer.
NO_LAYER = "simulation"


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


def describe(failure, blob, run_url=None):
    """The Markdown report of one failure."""
    lines = [
        f"### {title(failure['layer'], failure['law'])}",
        "",
        f"- Detail: {failure['detail']}",
        f"- Seed: `{failure.get('seed') or 'none'}`",
        f"- Replay: `{failure['replay']}`",
    ]
    if blob:
        lines.append(f'- Blob: `#[hegel::reproduce_failure("{blob}")]`')
    else:
        lines.append("- Blob: not printed; the case was not reproduced")
    if run_url:
        lines.append(f"- Run: {run_url}")
    lines.append(f"- What to do: {GUIDE}")
    lines += ["", "Shrunk steps:", "", "```json", json.dumps(failure["case"], indent=2), "```", ""]
    return "\n".join(lines)


def describe_run(layer, log="", run_url=None):
    """The report of a job that did not succeed with no other report."""
    lines = [
        f"### {title(layer, NO_LAW)}",
        "",
        "The job did not succeed, and no random case wrote a failure file. "
        "The build, a probe test or a planted test failed, a test panicked "
        "outside a law, or the job stopped at its time limit. Read the log.",
    ]
    if run_url:
        lines.append(f"Run: {run_url}")
    lines.append(f"What to do: {GUIDE}")
    return "\n".join(lines) + "\n"


def describe_check_broken(layer, log, run_url=None):
    """The report of the planted tests of `layer` whose law did not see the
    planted fault."""
    broken = [line.strip() for line in log.splitlines() if CHECK_BROKEN in line]
    lines = [
        f"### {title(layer, CHECK_BROKEN)}",
        "",
        "A planted test failed: its law did not see the planted fault, so the "
        "check itself is broken. A law issue of this run for the same layer "
        "is reported apart from this issue. The lines of the log:",
        "",
    ]
    lines += [f"- `{line}`" for line in dict.fromkeys(broken)]
    lines.append("")
    if run_url:
        lines.append(f"Run: {run_url}")
    lines.append(f"What to do: {GUIDE}")
    return "\n".join(lines) + "\n"


def probe_title(layer, test, law):
    """The title of a probe report; `law` is None for a check that is not a
    law."""
    if law is None:
        return f"Simulation: probe {test} failed a check in the {layer} layer"
    return f"Simulation: probe {test} broke law {law} in the {layer} layer"


def probe_failures(log, layer):
    """(test, law, detail, replay) of each probe of `layer` that broke a law
    or failed a check, once for each probe and law. The law of a failed check
    is None."""
    found = {}
    for match in PROBE_LINE.finditer(log):
        test, law, probe_layer, detail, replay = match.groups()
        if probe_layer == layer:
            found.setdefault((test, law), (test, law, detail, replay))
    return list(found.values())


def describe_probe(layer, probe, run_url=None):
    test, law, detail, replay = probe
    lines = [
        f"### {probe_title(layer, test, law)}",
        "",
        f"- Probe: `{test}`, a fixed case",
        f"- Detail: {detail}",
        f"- Replay: `{replay}`",
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


def layer_reports(layer, log, failures, run_url=None):
    """(title, body) of each report of one layer that did not succeed: one
    for each law of a random case, one for each probe and law, and one when a
    planted check is broken. With none of these, one that says that the
    layer failed without a broken law."""
    found = []
    for failure in layer_failures(log, layer, failures):
        body = describe(failure, blob_for(log, failure), run_url)
        found.append((title(layer, failure["law"]), body))
    for probe in probe_failures(log, layer):
        test, law = probe[0], probe[1]
        found.append((probe_title(layer, test, law), describe_probe(layer, probe, run_url)))
    if CHECK_BROKEN in log:
        found.append((title(layer, CHECK_BROKEN), describe_check_broken(layer, log, run_url)))
    if not found:
        found.append((title(layer, NO_LAW), describe_run(layer, log, run_url)))
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


def summary(layer, log_path, failures_dir, stats_dir=None, succeeded=False):
    text = counts(stats_dir, layer)
    if succeeded:
        return text or f"The {layer} layer passed.\n"
    log = Path(log_path).read_text(errors="replace") if Path(log_path).is_file() else ""
    found = layer_reports(layer, log, load_failures(failures_dir))
    return text + "\n".join(body for _, body in found)


def reports(artifacts, run_url):
    """(title, body) for each report of each layer.

    Each job uploads `<layer>.log` with its `failures` directory beside it.
    A download of one artifact unpacks it in `artifacts`; a download of more
    puts each in its own directory. So a layer is found from its log.
    """
    found = []
    for log_path in sorted(Path(artifacts).rglob("*.log")):
        layer = log_path.stem
        log = log_path.read_text(errors="replace")
        failures = load_failures(log_path.parent / "failures")
        found += layer_reports(layer, log, failures, run_url)
    if not found:
        found.append((title(NO_LAYER, NO_LAW), describe_run(NO_LAYER, "", run_url)))
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
