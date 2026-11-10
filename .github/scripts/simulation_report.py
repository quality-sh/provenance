#!/usr/bin/env python3
"""Report the broken laws of a simulation run.

`summary` writes Markdown for the step summary of one layer. `issues` opens
one issue per layer and law with the label `simulation`, or comments on the
open issue with the same title. A failure gives the law, the seed, the shrunk
steps, the replay line and the `#[hegel::reproduce_failure]` blob.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

BLOB = re.compile(r'#\[hegel::reproduce_failure\("([^"]*)"\)\]')
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
    return f"Simulation: law {law} broken in the {layer} layer"


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
    lines += ["", "Shrunk steps:", "", "```json", json.dumps(failure["case"], indent=2), "```", ""]
    return "\n".join(lines)


def describe_run(layer, run_url=None):
    lines = [f"### {title(layer, NO_LAW)}", "", "The job failed before a law was checked."]
    if run_url:
        lines.append(f"Run: {run_url}")
    return "\n".join(lines) + "\n"


def summary(layer, log_path, failures_dir):
    log = Path(log_path).read_text(errors="replace") if Path(log_path).is_file() else ""
    failures = load_failures(failures_dir)
    if not failures:
        return describe_run(layer)
    return "\n".join(describe(failure, blob_for(log, failure)) for failure in failures)


def reports(artifacts, run_url):
    """(title, body) for each failure of each layer.

    Each job uploads `<layer>.log` with its `failures` directory beside it.
    A download of one artifact unpacks it in `artifacts`; a download of more
    puts each in its own directory. So a layer is found from its log.
    """
    found = []
    for log_path in sorted(Path(artifacts).rglob("*.log")):
        layer = log_path.stem
        log = log_path.read_text(errors="replace")
        failures = [
            failure for failure in load_failures(log_path.parent / "failures")
            if failure["layer"] == layer
        ]
        if not failures:
            found.append((title(layer, NO_LAW), describe_run(layer, run_url)))
        for failure in failures:
            body = describe(failure, blob_for(log, failure), run_url)
            found.append((title(layer, failure["law"]), body))
    if not found:
        found.append((title(NO_LAYER, NO_LAW), describe_run(NO_LAYER, run_url)))
    return found


def gh(*args):
    return subprocess.run(["gh", *args], check=True, text=True, capture_output=True).stdout


def open_issues(artifacts, run_url, run=gh):
    run("label", "create", LABEL, "--force", "--color", "B60205",
        "--description", "A law of the nightly simulation broke")
    for issue_title, body in reports(artifacts, run_url):
        listed = json.loads(run("issue", "list", "--label", LABEL, "--state", "open",
                                "--search", f'"{issue_title}" in:title',
                                "--json", "number,title"))
        same = [issue for issue in listed if issue["title"] == issue_title]
        if same:
            run("issue", "comment", str(same[0]["number"]), "--body", body)
        else:
            run("issue", "create", "--title", issue_title, "--label", LABEL, "--body", body)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    one = commands.add_parser("summary")
    one.add_argument("--layer", required=True)
    one.add_argument("--log", required=True)
    one.add_argument("--failures", required=True)
    every = commands.add_parser("issues")
    every.add_argument("--artifacts", required=True)
    every.add_argument("--run-url", required=True)
    args = parser.parse_args(argv)
    if args.command == "summary":
        sys.stdout.write(summary(args.layer, args.log, args.failures))
    else:
        open_issues(args.artifacts, args.run_url)
    return 0


if __name__ == "__main__":
    sys.exit(main())
