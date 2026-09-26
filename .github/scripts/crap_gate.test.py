#!/usr/bin/env python3

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("crap_gate.py")
FIXTURES = Path(__file__).parent / "fixtures" / "crap_gate"
WORKFLOW = Path(__file__).parents[1] / "workflows" / "ci.yml"
PASSING_LCOV = """\
SF:.github/scripts/fixtures/crap_gate/passing/src/lib.rs
FN:1,well_tested
FNDA:1,well_tested
DA:1,1
DA:2,1
DA:3,1
end_of_record
"""
FAILING_LCOV = """\
SF:.github/scripts/fixtures/crap_gate/failing/src/lib.rs
FN:1,complex_uncovered
FNDA:0,complex_uncovered
DA:1,0
DA:2,0
DA:3,0
DA:4,0
DA:5,0
DA:6,0
DA:7,0
DA:8,0
DA:9,0
DA:10,0
DA:11,0
DA:12,0
DA:13,0
DA:14,0
DA:15,0
end_of_record
"""


def load_gate():
    spec = importlib.util.spec_from_file_location("crap_gate", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def report(entries, diagnostics=None, version="0.5.0"):
    return {
        "version": version,
        "entries": entries,
        "diagnostics": diagnostics
        or {
            "analyzed_files": 1,
            "lcov_files": 1,
            "matched_files": 1,
            "source_only": {"count": 0, "examples": []},
            "lcov_only": {"count": 0, "examples": []},
        },
    }


def entry(coverage=50.0, crap=2.125):
    return {
        "file": "src/lib.rs",
        "function": "example",
        "line": 1,
        "cyclomatic": 2.0,
        "coverage": coverage,
        "crap": crap,
    }


class ReportValidationTests(unittest.TestCase):
    def setUp(self):
        self.gate = load_gate()
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.lcov = self.root / "lcov.info"
        self.lcov.write_text(
            "SF:src/lib.rs\nFN:1,example\nFNDA:1,example\nDA:2,1\nend_of_record\n"
        )

    def tearDown(self):
        self.temp.cleanup()

    def test_simple_executed_function_is_valid(self):
        self.gate.validate_report(report([entry(100.0, 2.0)]), self.lcov, "0.5.0")

    def test_empty_analysis_cannot_pass(self):
        with self.assertRaisesRegex(self.gate.GateInputError, "no functions"):
            self.gate.validate_report(report([]), self.lcov, "0.5.0")

    def test_empty_coverage_cannot_pass(self):
        self.lcov.write_text("")
        with self.assertRaisesRegex(self.gate.GateInputError, "no LCOV source records"):
            self.gate.validate_report(report([entry()]), self.lcov, "0.5.0")

    def test_coverage_without_line_instrumentation_cannot_pass(self):
        self.lcov.write_text(
            "SF:src/lib.rs\nFN:1,example\nFNDA:1,example\nend_of_record\n"
        )
        with self.assertRaisesRegex(self.gate.GateInputError, "no LCOV line records"):
            self.gate.validate_report(report([entry(100.0, 2.0)]), self.lcov, "0.5.0")

    def test_missing_coverage_input_cannot_pass(self):
        self.lcov.unlink()
        with self.assertRaisesRegex(self.gate.GateInputError, "does not exist"):
            self.gate.validate_report(report([entry()]), self.lcov, "0.5.0")

    def test_malformed_report_cannot_pass(self):
        with self.assertRaisesRegex(self.gate.GateInputError, "not a JSON object"):
            self.gate.validate_report([], self.lcov, "0.5.0")

    def test_mismatched_coverage_cannot_pass(self):
        diagnostics = {
            "analyzed_files": 1,
            "lcov_files": 1,
            "matched_files": 0,
            "source_only": {"count": 1, "examples": ["src/lib.rs"]},
            "lcov_only": {"count": 1, "examples": ["elsewhere.rs"]},
        }
        with self.assertRaisesRegex(self.gate.GateInputError, "no source files match"):
            self.gate.validate_report(report([entry(None, 6.0)], diagnostics), self.lcov, "0.5.0")

    def test_missing_file_coverage_is_pessimistic_but_cannot_pass(self):
        missing = entry(None, 6.0)
        covered = entry(50.0, 2.125)
        covered["function"] = "covered_neighbor"
        with self.assertRaisesRegex(self.gate.GateInputError, "missing LCOV coverage"):
            self.gate.validate_report(report([missing, covered]), self.lcov, "0.5.0")

    def test_source_only_files_cannot_pass(self):
        diagnostics = {
            "analyzed_files": 2,
            "lcov_files": 1,
            "matched_files": 1,
            "source_only": {"count": 1, "examples": ["src/platform.rs"]},
            "lcov_only": {"count": 0, "examples": []},
        }
        with self.assertRaisesRegex(self.gate.GateInputError, "1 analyzed source file"):
            self.gate.validate_report(report([entry()], diagnostics), self.lcov, "0.5.0")

    def test_uninstrumented_function_cannot_look_fully_covered(self):
        self.lcov.write_text("SF:src/lib.rs\nDA:99,1\nend_of_record\n")
        with self.assertRaisesRegex(self.gate.GateInputError, "no executed LCOV function record"):
            self.gate.validate_report(report([entry(100.0, 2.0)]), self.lcov, "0.5.0")

    def platform_row(self, attribute):
        source = self.root / "platform.rs"
        source.write_text(f"{attribute}\npub(super) fn example() {{}}\n")
        self.lcov.write_text(f"SF:{source}\nDA:99,1\nend_of_record\n")
        row = entry(100.0, 2.0)
        row["file"] = str(source)
        row["line"] = 2
        return row

    def test_platform_only_function_is_rejected_by_default(self):
        row = self.platform_row("#[cfg(windows)]")
        with self.assertRaisesRegex(self.gate.GateInputError, "no executed LCOV function record"):
            self.gate.validate_report(report([row]), self.lcov, "0.5.0")

    def test_platform_only_function_is_listed_when_allowed(self):
        row = self.platform_row("#[cfg(windows)]")
        allowed = self.gate.validate_report(report([row]), self.lcov, "0.5.0", True)
        self.assertEqual(allowed, [row])

    def test_linux_function_without_record_is_rejected_when_allowed(self):
        row = self.platform_row("#[cfg(any(windows, unix))]")
        with self.assertRaisesRegex(self.gate.GateInputError, "no executed LCOV function record"):
            self.gate.validate_report(report([row]), self.lcov, "0.5.0", True)

    def test_same_filename_in_two_crates_is_not_guessed(self):
        self.lcov.write_text(
            "SF:crates/alpha/src/lib.rs\n"
            "FN:1,alpha\nFNDA:1,alpha\nDA:1,1\nend_of_record\n"
            "SF:crates/beta/src/lib.rs\n"
            "FN:1,beta\nFNDA:1,beta\nDA:1,1\nend_of_record\n"
        )
        ambiguous = entry(100.0, 2.0)
        ambiguous["file"] = "src/lib.rs"
        with self.assertRaisesRegex(self.gate.GateInputError, "ambiguous LCOV paths"):
            self.gate.validate_report(report([ambiguous]), self.lcov, "0.5.0")

    def test_report_must_come_from_pinned_tool(self):
        with self.assertRaisesRegex(self.gate.GateInputError, "expected cargo-crap 0.5.0"):
            self.gate.validate_report(report([entry()], version="0.6.0"), self.lcov, "0.5.0")


class NativeGateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tool = os.environ.get("CARGO_CRAP_BIN") or shutil.which("cargo-crap")
        if not cls.tool:
            raise unittest.SkipTest("cargo-crap is not installed")

    def run_fixture(self, name):
        with tempfile.TemporaryDirectory() as temp:
            lcov = Path(temp) / "lcov.info"
            lcov.write_text(PASSING_LCOV if name == "passing" else FAILING_LCOV)
            command = [
                "python3",
                str(SCRIPT),
                "--cargo-crap",
                self.tool,
                "--path",
                str(FIXTURES / name / "src"),
                "--lcov",
                str(lcov),
                "--report",
                str(Path(temp) / "report.json"),
                "--threshold",
                "30",
            ]
            result = subprocess.run(command, text=True, capture_output=True, check=False)
            parsed = json.loads((Path(temp) / "report.json").read_text())
            return result, parsed

    def test_simple_well_tested_code_passes(self):
        result, parsed = self.run_fixture("passing")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        row = parsed["entries"][0]
        self.assertEqual(row["function"], "well_tested")
        self.assertEqual(row["coverage"], 100.0)
        self.assertEqual(row["crap"], row["cyclomatic"])
        self.assertIn("CC", result.stdout)
        self.assertIn("100.0%", result.stdout)

    def test_complex_insufficiently_tested_code_fails_with_raw_metrics(self):
        result, parsed = self.run_fixture("failing")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        row = parsed["entries"][0]
        self.assertGreater(row["crap"], 30)
        self.assertGreater(row["cyclomatic"], 1)
        self.assertEqual(row["coverage"], 0.0)
        self.assertIn("CRAP threshold 30", result.stdout)
        self.assertIn("complex_uncovered", result.stdout)

    def test_invalid_coverage_still_prints_native_threshold_candidates(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            lcov = root / "lcov.info"
            lcov.write_text(
                f"SF:{FIXTURES / 'failing' / 'src' / 'lib.rs'}\n"
                "DA:99,1\nend_of_record\n"
            )
            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--cargo-crap",
                    self.tool,
                    "--path",
                    str(FIXTURES / "failing" / "src"),
                    "--lcov",
                    str(lcov),
                    "--report",
                    str(root / "report.json"),
                    "--threshold",
                    "5",
                ],
                text=True,
                capture_output=True,
                check=False,
            )
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("complex_uncovered", result.stdout)
        self.assertIn("CC", result.stdout)
        self.assertIn("100.0%", result.stdout)
        self.assertIn("no executed LCOV function record", result.stderr)

    def test_real_workspace_attribution_is_explicit(self):
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "real-workspace.json"
            subprocess.run(
                [self.tool, "--workspace", "--format", "json", "--output", str(output)],
                check=True,
            )
            rows = json.loads(output.read_text())["entries"]
        by_name = {row["function"]: row for row in rows}
        method = by_name["Settings::from_env"]
        self.assertEqual(method["crate"], "provenance-sdk")
        self.assertTrue(method["file"].endswith("crates/provenance-sdk/src/settings.rs"))
        self.assertEqual(by_name["Execution::run"]["cyclomatic"], 4.0)
        self.assertFalse(any("closure" in name for name in by_name))
        self.assertFalse(
            any(row["file"].endswith("operations/catalog/creation.rs") for row in rows),
            "functions created by macro expansion are opaque to cargo-crap",
        )


class WorkflowWiringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = WORKFLOW.read_text()
        cls.job = cls.workflow.split("  crap-quality:", 1)[1].split("\n  rule-coverage:", 1)[0]

    def test_quality_inputs_trigger_the_rust_jobs(self):
        rust_filter = self.workflow.split("            rust:", 1)[1].split(
            "            contract:", 1
        )[0]
        self.assertIn(".github/scripts/crap_gate", rust_filter)

    def test_job_runs_executed_workspace_tests_with_pinned_tools(self):
        self.assertIn("cargo-llvm-cov@0.9.1", self.job)
        self.assertIn("cargo-crap@0.5.0", self.job)
        self.assertIn("mkdir -p target/crap", self.job)
        self.assertIn("cargo llvm-cov --workspace --all-features --lcov", self.job)
        self.assertNotIn("continue-on-error", self.job)
        self.assertNotIn("PROVENANCE_CI_LOCAL", self.job)

    def test_job_runs_native_fixtures_and_fail_closed_gate(self):
        self.assertIn("python3 .github/scripts/crap_gate.test.py", self.job)
        self.assertIn("python3 .github/scripts/crap_gate.py", self.job)
        self.assertIn("--threshold 30", self.job)
        self.assertIn("--allow-unmeasured-platform-code", self.job)
        self.assertIn("--workspace", self.job)
        self.assertIn("--exclude 'build.rs'", self.job)
        self.assertIn("--exclude 'build/**'", self.job)
        self.assertIn("--exclude 'src/fixture.rs'", self.job)
        self.assertIn("--exclude 'src/bin/*-fixture.rs'", self.job)
        self.assertIn("if: always()", self.job)
        self.assertIn("crap-report.json", self.job)

    def test_ci_ok_requires_crap_quality(self):
        ci_ok = self.workflow.split("  ci-ok:", 1)[1]
        self.assertIn("- crap-quality", ci_ok)


if __name__ == "__main__":
    unittest.main()
