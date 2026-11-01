#!/usr/bin/env python3
"""Tests for select_tests.py against the metadata of this workspace.

SELECT_TESTS_METADATA names a file with the output of
`cargo metadata --format-version 1 --no-deps`.
"""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("select_tests.py")
WORKFLOW = Path(__file__).parents[1] / "workflows" / "ci.yml"


def load_selector():
    spec = importlib.util.spec_from_file_location("select_tests", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SELECTOR = load_selector()
METADATA = json.loads(Path(os.environ["SELECT_TESTS_METADATA"]).read_text())
ALL_CRATES = sorted(package["name"] for package in METADATA["packages"])


def select(*files, event="pull_request"):
    return SELECTOR.select(event, list(files), METADATA)


class SelectionTests(unittest.TestCase):
    def assert_full_suite(self, selection):
        self.assertEqual(selection["test-scope"], "all")
        self.assertEqual(selection["test-packages"], "--workspace")
        self.assertEqual(selection["test-filter"], "all()")
        self.assertEqual(selection["doc-packages"], "--workspace")
        self.assertEqual(json.loads(selection["test-crates"]), ALL_CRATES)

    def test_events_other_than_pull_request_run_the_full_suite(self):
        for event in ("push", "workflow_dispatch", "schedule"):
            with self.subTest(event=event):
                self.assert_full_suite(select("docs/guide.md", event=event))

    def test_workspace_wide_files_run_the_full_suite(self):
        for path in (
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo/config.toml",
            ".config/nextest.toml",
            ".github/workflows/ci.yml",
            ".github/scripts/select_tests.py",
            ".github/scripts/crap_gate.py",
        ):
            with self.subTest(path=path):
                self.assert_full_suite(select("crates/provenance-cli/src/lib.rs", path))

    def test_an_unknown_file_runs_the_full_suite(self):
        self.assert_full_suite(select("tests/fixtures/merge/new.json"))
        self.assert_full_suite(select("crates/not-a-crate/src/lib.rs"))

    def test_documentation_selects_no_tests(self):
        selection = select("docs/guide.md", "AGENTS.md", "examples/typescript-sdk/index.ts")
        self.assertEqual(selection["test-scope"], "none")
        self.assertEqual(selection["test-packages"], "")
        self.assertEqual(json.loads(selection["test-crates"]), [])

    def test_a_cli_change_selects_the_cli_and_its_source_readers(self):
        selection = select("crates/provenance-cli/src/lib.rs")
        self.assertEqual(selection["test-scope"], "some")
        self.assertEqual(json.loads(selection["test-crates"]), ["provenance-cli"])
        self.assertEqual(selection["doc-packages"], "-p provenance-cli")
        self.assertEqual(selection["crate-dirs"], "crates/provenance-cli")
        # The wildcard gate in provenance-core reads the CLI source.
        self.assertEqual(
            selection["test-packages"], "-p provenance-cli -p provenance-core"
        )
        self.assertEqual(
            selection["test-filter"],
            "rdeps(=provenance-cli) | binary_id(provenance-cli::cli_structure)"
            " | binary_id(provenance-core::no_wildcard_gates)",
        )

    def test_a_store_change_selects_every_crate_that_depends_on_the_store(self):
        selection = select("crates/provenance-store/src/lib.rs")
        crates = json.loads(selection["test-crates"])
        for crate in (
            "provenance-store",
            "provenance-cli",
            "provenance-transport",
            "provenance-contract",
            "provenance-report",
            "provenance-sdk",
            "provenance-codegen",
        ):
            self.assertIn(crate, crates)
        for crate in ("provenance-core", "provenance-macros", "provenance-http-client"):
            self.assertNotIn(crate, crates)
        self.assertTrue(selection["test-filter"].startswith("rdeps(=provenance-store)"))

    def test_dev_dependencies_count_as_dependents(self):
        # provenance-macros has provenance-scanner only as a dev dependency.
        crates = json.loads(select("crates/provenance-scanner/src/lib.rs")["test-crates"])
        self.assertIn("provenance-macros", crates)

    def test_root_inputs_select_the_crates_that_read_them(self):
        selection = select("skills/provenance/SKILL.md")
        self.assertEqual(json.loads(selection["test-crates"]), ["provenance-cli"])
        selection = select("contracts/operations/statements.json")
        self.assertEqual(json.loads(selection["test-crates"]), ["provenance-http-client"])

    def test_a_rust_file_outside_the_crates_runs_only_the_file_audit(self):
        selection = select("examples/rust-sdk/src/main.rs")
        self.assertEqual(selection["test-scope"], "some")
        self.assertEqual(json.loads(selection["test-crates"]), [])
        self.assertEqual(selection["test-packages"], "-p provenance-cli")
        self.assertEqual(selection["test-filter"], "binary_id(provenance-cli::cli_structure)")
        self.assertEqual(selection["crate-dirs"], "")

    def test_every_named_crate_and_test_binary_exists(self):
        names = set(ALL_CRATES)
        for _, crates in SELECTOR.ROOT_INPUTS:
            self.assertLessEqual(set(crates), names)
        root = Path(METADATA["workspace_root"])
        directories, _ = SELECTOR.workspace(METADATA)
        for _, _, binary in SELECTOR.SOURCE_READERS:
            crate, target = binary.split("::", 1)
            self.assertIn(crate, names)
            self.assertTrue((root / directories[crate] / "tests" / f"{target}.rs").is_file(), binary)

    def test_the_command_line_writes_workflow_outputs(self):
        with tempfile.TemporaryDirectory() as temp:
            files = Path(temp) / "files.json"
            files.write_text(json.dumps(["docs/guide.md"]))
            output = Path(temp) / "output"
            subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--event",
                    "pull_request",
                    "--changed-files",
                    str(files),
                    "--metadata",
                    os.environ["SELECT_TESTS_METADATA"],
                    "--output",
                    str(output),
                ],
                check=True,
                capture_output=True,
            )
            self.assertIn("test-scope=none\n", output.read_text())


class WorkflowWiringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = WORKFLOW.read_text()

    def job(self, name, following):
        return self.workflow.split(f"\n  {name}:\n", 1)[1].split(f"\n  {following}:", 1)[0]

    def test_the_changes_job_tests_and_runs_the_selector(self):
        changes = self.job("changes", "generate-operations")
        self.assertIn("python3 .github/scripts/select_tests.test.py", changes)
        self.assertIn("python3 .github/scripts/ci_ok.test.py", changes)
        self.assertIn("python3 .github/scripts/select_tests.py", changes)
        for output in ("test-scope", "test-crates", "test-packages", "test-filter"):
            self.assertIn(f"{output}: ${{{{ steps.select.outputs.{output} }}}}", changes)

    def test_the_test_job_runs_only_the_selected_tests(self):
        test = self.job("test", "test-default-features")
        self.assertIn("needs.changes.outputs.test-scope != 'none'", test)
        self.assertIn('read -ra packages <<< "$TEST_PACKAGES"', test)
        self.assertIn('cargo llvm-cov nextest "${packages[@]}"', test)
        self.assertIn('-E "($TEST_FILTER) & not ($COMPILE_FAIL_TESTS)"', test)

    def test_the_cli_jobs_need_the_cli_in_the_selection(self):
        for name, following in (
            ("test-default-features", "doc-tests"),
            ("skills-install", "ci-ok"),
        ):
            job = self.job(name, following)
            self.assertIn(
                "contains(fromJSON(needs.changes.outputs.test-crates), 'provenance-cli')", job
            )

    def test_the_compile_fail_job_has_its_own_path_filter(self):
        filters = self.workflow.split("            compile-fail:", 1)[1].split(
            "            rust:", 1
        )[0]
        for path in (
            "crates/provenance-store/**",
            "crates/provenance-core/**",
            "crates/provenance-macros/**",
            "crates/provenance-sdk/**",
            "Cargo.lock",
            "rust-toolchain.toml",
        ):
            self.assertIn(path, filters)
        job = self.job("compile-fail", "crap-quality")
        self.assertIn("needs.changes.outputs.compile-fail == 'true'", job)


if __name__ == "__main__":
    unittest.main()
