#!/usr/bin/env python3
"""Run cargo-crap and reject incomplete coverage inputs before its threshold gate."""

import argparse
import json
from pathlib import Path
import subprocess
import sys


class GateInputError(RuntimeError):
    """An analysis input or output cannot support an enforcing result."""


def _number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _suffix_matches(left, right):
    left_parts = Path(left).parts
    right_parts = Path(right).parts
    shorter = min(len(left_parts), len(right_parts))
    return left_parts[-shorter:] == right_parts[-shorter:]


def _matching_functions(lcov, report_path):
    candidates = [
        (min(len(Path(path).parts), len(Path(report_path).parts)), path, functions)
        for path, functions in lcov.items()
        if _suffix_matches(path, report_path)
    ]
    if not candidates:
        return []
    specificity = max(candidate[0] for candidate in candidates)
    best = [candidate for candidate in candidates if candidate[0] == specificity]
    if len(best) != 1:
        paths = ", ".join(sorted(candidate[1] for candidate in best))
        raise GateInputError(f"ambiguous LCOV paths for {report_path}: {paths}")
    return best[0][2]


def _read_lcov(path):
    if not path.is_file():
        raise GateInputError(f"LCOV input does not exist: {path}")
    try:
        lines = path.read_text().splitlines()
    except OSError as error:
        raise GateInputError(f"cannot read LCOV input {path}: {error}") from error

    records = {}
    line_records = 0
    source = None
    declared = []
    hits = {}
    for raw in [*lines, "end_of_record"]:
        if raw.startswith("SF:"):
            if source is not None:
                raise GateInputError(f"LCOV record for {source} has no end_of_record")
            source = raw[3:]
            declared = []
            hits = {}
        elif raw.startswith("FN:") and source is not None:
            fields = raw[3:].split(",", 1)
            if len(fields) != 2:
                raise GateInputError(f"invalid LCOV FN record: {raw}")
            try:
                line = int(fields[0])
            except ValueError as error:
                raise GateInputError(f"invalid LCOV function line: {raw}") from error
            declared.append((line, fields[1]))
        elif raw.startswith("FNDA:") and source is not None:
            fields = raw[5:].split(",", 1)
            if len(fields) != 2:
                raise GateInputError(f"invalid LCOV FNDA record: {raw}")
            try:
                count = int(fields[0])
            except ValueError as error:
                raise GateInputError(f"invalid LCOV function count: {raw}") from error
            hits[fields[1]] = hits.get(fields[1], 0) + count
        elif raw.startswith("DA:") and source is not None:
            line_records += 1
        elif raw == "end_of_record" and source is not None:
            records[source] = [
                (line, name, hits.get(name, 0)) for line, name in declared
            ]
            source = None
            declared = []
            hits = {}

    if not records:
        raise GateInputError("LCOV input has no LCOV source records")
    if line_records == 0:
        raise GateInputError("LCOV input has no LCOV line records")
    return records


def _validate_entry(row):
    location = f"{row.get('file', '<unknown>')}:{row.get('line', '?')}"
    function = row.get("function")
    if not isinstance(row.get("file"), str) or not row["file"]:
        raise GateInputError("cargo-crap entry has no file")
    if not isinstance(function, str) or not function:
        raise GateInputError(f"cargo-crap entry at {location} has no function")
    if not isinstance(row.get("line"), int) or row["line"] < 1:
        raise GateInputError(f"cargo-crap entry for {function} has an invalid line")
    if not _number(row.get("cyclomatic")) or row["cyclomatic"] < 1:
        raise GateInputError(f"cargo-crap entry for {function} has invalid cyclomatic complexity")
    coverage = row.get("coverage")
    if coverage is not None and (not _number(coverage) or not 0 <= coverage <= 100):
        raise GateInputError(f"cargo-crap entry for {function} has invalid coverage")
    if not _number(row.get("crap")) or row["crap"] < 1:
        raise GateInputError(f"cargo-crap entry for {function} has an invalid CRAP score")
    if coverage is None:
        pessimistic = row["cyclomatic"] ** 2 + row["cyclomatic"]
        if abs(row["crap"] - pessimistic) > 0.01:
            raise GateInputError(
                f"missing coverage for {function} at {location} must use 0% coverage"
            )


def _validate_diagnostics(report):
    diagnostics = report.get("diagnostics")
    if not isinstance(diagnostics, dict):
        raise GateInputError("cargo-crap report has no LCOV scope diagnostics")
    for key in ("analyzed_files", "lcov_files", "matched_files"):
        if not isinstance(diagnostics.get(key), int) or diagnostics[key] < 0:
            raise GateInputError(f"cargo-crap diagnostics have invalid {key}")
    if diagnostics["analyzed_files"] == 0:
        raise GateInputError("cargo-crap diagnostics report no analyzed files")
    if diagnostics["lcov_files"] == 0:
        raise GateInputError("cargo-crap diagnostics report no LCOV files")
    if diagnostics["matched_files"] == 0:
        raise GateInputError("cargo-crap diagnostics show no source files match LCOV")
    source_only = diagnostics.get("source_only")
    if not isinstance(source_only, dict) or not isinstance(source_only.get("count"), int):
        raise GateInputError("cargo-crap diagnostics have invalid source_only data")
    if source_only["count"]:
        raise GateInputError(
            f"{source_only['count']} analyzed source file(s) have no LCOV coverage"
        )


def _validate_perfect_coverage(entries, lcov):
    for row in entries:
        if row["coverage"] != 100:
            continue
        functions = _matching_functions(lcov, row["file"])
        executed_at_line = any(line == row["line"] and count > 0 for line, _, count in functions)
        if not executed_at_line:
            raise GateInputError(
                f"{row['function']} at {row['file']}:{row['line']} reports 100% coverage "
                "but has no executed LCOV function record"
            )


def validate_report(report, lcov_path, expected_version):
    """Reject reports that could make absent analysis data look green."""
    if not isinstance(report, dict):
        raise GateInputError("cargo-crap report is not a JSON object")
    if report.get("version") != expected_version:
        raise GateInputError(
            f"expected cargo-crap {expected_version}, got {report.get('version', '<missing>')}"
        )
    entries = report.get("entries")
    if not isinstance(entries, list):
        raise GateInputError("cargo-crap report has no entries array")
    if not entries:
        raise GateInputError("cargo-crap analyzed no functions")
    _validate_diagnostics(report)
    for row in entries:
        if not isinstance(row, dict):
            raise GateInputError("cargo-crap report contains a non-object entry")
        _validate_entry(row)
    missing = [row for row in entries if row["coverage"] is None]
    if missing:
        first = missing[0]
        raise GateInputError(
            f"{len(missing)} function(s) have missing LCOV coverage; first is "
            f"{first['function']} at {first['file']}:{first['line']}"
        )
    lcov = _read_lcov(Path(lcov_path))
    _validate_perfect_coverage(entries, lcov)


def _arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-crap", default="cargo-crap")
    scope = parser.add_mutually_exclusive_group(required=True)
    scope.add_argument("--workspace", action="store_true")
    scope.add_argument("--path")
    parser.add_argument("--lcov", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--threshold", type=float, required=True)
    parser.add_argument("--expected-version", default="0.5.0")
    parser.add_argument("--exclude", action="append", default=[])
    return parser.parse_args()


def _tool_args(args):
    command = [args.cargo_crap]
    command.append("--workspace" if args.workspace else "--path")
    if not args.workspace:
        command.append(args.path)
    command.extend(
        [
            "--lcov",
            str(args.lcov),
            "--missing",
            "pessimistic",
            "--threshold",
            str(args.threshold),
        ]
    )
    for pattern in args.exclude:
        command.extend(["--exclude", pattern])
    return command


def main():
    args = _arguments()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    base = _tool_args(args)
    version = subprocess.run(
        [args.cargo_crap, "--version"], text=True, capture_output=True, check=False
    )
    if version.returncode != 0 or args.expected_version not in version.stdout.split():
        print(
            f"CRAP gate input error: expected cargo-crap {args.expected_version}; "
            f"version command returned {version.stdout.strip() or version.stderr.strip()}",
            file=sys.stderr,
        )
        return 2
    report_run = subprocess.run(
        [*base, "--format", "json", "--output", str(args.report)], check=False
    )
    if report_run.returncode != 0:
        print("CRAP gate input error: cargo-crap did not produce a report", file=sys.stderr)
        return report_run.returncode or 2
    try:
        parsed = json.loads(args.report.read_text())
        validate_report(parsed, args.lcov, args.expected_version)
    except (OSError, json.JSONDecodeError, GateInputError) as error:
        print(f"CRAP gate input error: {error}", file=sys.stderr)
        return 2

    print(
        "Policy: CRAP = C*C*(1-coverage)^3+C; "
        f"fail when CRAP > {args.threshold:g}. Top 50 raw results follow.",
        flush=True,
    )
    top = subprocess.run([*base, "--format", "human", "--top", "50"])
    if top.returncode != 0:
        return top.returncode
    print(
        f"Showing every function with CRAP >= {args.threshold:g}; "
        f"cargo-crap fails only scores above {args.threshold:g}.",
        flush=True,
    )
    result = subprocess.run(
        [*base, "--format", "human", "--min", str(args.threshold), "--fail-above"]
    )
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
