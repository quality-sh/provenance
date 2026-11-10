#!/usr/bin/env python3
"""Tests for select_tests.py against the metadata of this workspace.

SELECT_TESTS_METADATA names a file with the output of
`cargo metadata --format-version 1 --no-deps`.
"""

import importlib.util
import json
import os
from pathlib import Path
import re
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


# Test binaries that read above their crate directory, but read only files
# that ROOT_INPUTS maps to their crate or files of crates that they depend on.
# A test that reads the Rust source of a crate that it does not depend on goes
# in RUST_SOURCE_READERS in select_tests.py instead.
ROOT_FILE_READERS = (
    "provenance-cli::ci_coverage_workflow",
    "provenance-cli::ci_wasm_kernel",
    "provenance-cli::cli_import_legacy_audit",
    "provenance-cli::cli_import_lifecycle",
    "provenance-cli::cli_skills",
    "provenance-cli::cli_skills_build_source",
    "provenance-cli::cli_skills_copy_tree",
    "provenance-cli::cli_skills_single_source",
    "provenance-cli::cli_version",
    "provenance-cli::release_commands",
    "provenance-cli::review_bundle",
    "provenance-scanner::scanner_declarations",
    # The unit tests of the store library read .provenance/state.
    "provenance-store",
    "provenance-store::review_cost",
)

# A test file that takes CARGO_MANIFEST_DIR and goes up from it reads files
# above its crate directory.
GOES_UP = re.compile(r"\.parent\(\)|\bancestors\(\)|\"\.\./|join\(\"\.\.\"\)")
INCLUDE = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"\s*\)')


def binaries_of_test_files(crate, crate_directory):
    """Return the test binaries that compile each test file of a crate.

    A file in src/ belongs to the library tests, which have the crate name as
    their binary id. Production files in src/ are left out.
    """
    tests = crate_directory / "tests"
    binaries = {}
    for path in sorted(crate_directory.rglob("*.rs")):
        relative = path.relative_to(crate_directory)
        if relative.parts[0] == "src":
            if "test" in path.name or "tests" in relative.parts:
                binaries[path] = [crate]
        elif relative.parts[0] == "tests":
            name = relative.parts[1].removesuffix(".rs")
            if (tests / f"{name}.rs").is_file():
                binaries[path] = [f"{crate}::{name}"]
            else:
                # A shared module directory: the binaries that name it own the file.
                names = re.compile(rf"\bmod {re.escape(name)}\b|/{re.escape(name)}/")
                binaries[path] = [
                    f"{crate}::{top.stem}"
                    for top in sorted(tests.glob("*.rs"))
                    if names.search(top.read_text())
                ]
    return binaries


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
        self.assertEqual(
            selection["test-packages"],
            "-p provenance-cli -p provenance-contract -p provenance-core"
            " -p provenance-scanner -p provenance-store",
        )
        self.assertEqual(
            selection["test-filter"],
            " | ".join(
                ["rdeps(=provenance-cli)"]
                + [f"binary_id({binary})" for binary in SELECTOR.RUST_SOURCE_READERS]
            ),
        )

    def test_a_rust_change_in_any_crate_selects_every_rust_source_reader(self):
        # PR 642 changed only CLI and transport files. A direct Git start in a
        # CLI test file broke the store test that reads every crate.
        for path in (
            "crates/provenance-cli/tests/cli_init_typescript_commands.rs",
            "crates/provenance-transport/src/skills.rs",
            "crates/provenance-macros/src/lib.rs",
            "crates/provenance-http-client/tests/client_policy.rs",
            "crates/provenance-cli/build/commit.rs",
        ):
            with self.subTest(path=path):
                selection = select(path)
                for binary in SELECTOR.RUST_SOURCE_READERS:
                    self.assertIn(f"binary_id({binary})", selection["test-filter"])
                    self.assertIn(f"-p {binary.split('::')[0]} ", selection["test-packages"] + " ")

    def test_a_file_that_is_not_rust_selects_no_rust_source_reader(self):
        selection = select("crates/provenance-cli/tests/fixtures/surface_names/cli.txt")
        self.assertNotIn("git_starts_use_the_helper", selection["test-filter"])
        self.assertNotIn("-p provenance-store", selection["test-packages"])

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

    def test_a_store_change_selects_the_scanner_test_that_reads_the_store(self):
        for path in (
            "crates/provenance-store/src/lib.rs",
            "crates/provenance-store/tests/shape.rs",
            "crates/provenance-store/README.md",
        ):
            with self.subTest(path=path):
                selection = select(path)
                self.assertIn(
                    "binary_id(provenance-scanner::scanner_lexer_regressions)",
                    selection["test-filter"],
                )
                self.assertIn("-p provenance-scanner", selection["test-packages"])

    def test_a_store_ignore_file_selects_the_scanner_test_that_reads_the_store(self):
        # The scan obeys these files and the text search in the test does not.
        for path in (
            "crates/provenance-store/.gitignore",
            "crates/provenance-store/src/.gitignore",
        ):
            with self.subTest(path=path):
                selection = select(path)
                self.assertIn(
                    "binary_id(provenance-scanner::scanner_lexer_regressions)",
                    selection["test-filter"],
                )
                self.assertIn("-p provenance-scanner", selection["test-packages"])

    def test_a_manifest_change_selects_the_tests_that_read_every_manifest(self):
        selection = select("crates/provenance-http-client/Cargo.toml")
        for binary in (
            "provenance-cli::crate_dependencies",
            "provenance-cli::release_structure",
            "provenance-cli::release_onboarding_structure",
        ):
            self.assertIn(f"binary_id({binary})", selection["test-filter"])
        self.assertIn("-p provenance-cli", selection["test-packages"])

    def test_dev_dependencies_count_as_dependents(self):
        # provenance-macros has provenance-scanner only as a dev dependency.
        crates = json.loads(select("crates/provenance-scanner/src/lib.rs")["test-crates"])
        self.assertIn("provenance-macros", crates)

    def test_root_inputs_select_the_crates_that_read_them(self):
        selection = select("skills/provenance/SKILL.md")
        self.assertEqual(
            json.loads(selection["test-crates"]),
            ["provenance-cli", "provenance-mcp", "provenance-review", "provenance-transport"],
        )
        selection = select("contracts/operations/statements.json")
        self.assertEqual(json.loads(selection["test-crates"]), ["provenance-http-client"])

    def test_a_rust_file_outside_the_crates_runs_only_the_source_readers(self):
        selection = select("examples/rust-sdk/src/main.rs")
        self.assertEqual(selection["test-scope"], "some")
        self.assertEqual(json.loads(selection["test-crates"]), [])
        crates = sorted({binary.split("::")[0] for binary in SELECTOR.RUST_SOURCE_READERS})
        self.assertEqual(selection["test-packages"], " ".join(f"-p {crate}" for crate in crates))
        self.assertEqual(
            selection["test-filter"],
            " | ".join(f"binary_id({binary})" for binary in SELECTOR.RUST_SOURCE_READERS),
        )
        self.assertEqual(selection["crate-dirs"], "")

    def test_every_named_crate_and_test_binary_exists(self):
        names = set(ALL_CRATES)
        for _, crates in SELECTOR.ROOT_INPUTS:
            self.assertLessEqual(set(crates), names)
        root = Path(METADATA["workspace_root"])
        directories, _ = SELECTOR.workspace(METADATA)
        binaries = [binary for _, _, binary in SELECTOR.SOURCE_READERS]
        binaries += SELECTOR.RUST_SOURCE_READERS + ROOT_FILE_READERS
        for binary in binaries:
            crate, _, target = binary.partition("::")
            self.assertIn(crate, names)
            if target:
                self.assertTrue(
                    (root / directories[crate] / "tests" / f"{target}.rs").is_file(), binary
                )

    def test_each_test_that_reads_above_its_crate_is_in_a_list(self):
        # A test that reads the files of another crate must run when the
        # files change. A new test of this kind fails here until a list names
        # it, so the selection cannot leave it out without a decision.
        root = Path(METADATA["workspace_root"])
        directories, _ = SELECTOR.workspace(METADATA)
        known = set(SELECTOR.RUST_SOURCE_READERS) | set(ROOT_FILE_READERS)
        known |= {binary for _, _, binary in SELECTOR.SOURCE_READERS}
        found = 0
        for crate, directory in sorted(directories.items()):
            for path, binaries in binaries_of_test_files(crate, root / directory).items():
                lines = [
                    line
                    for line in path.read_text().splitlines()
                    if not line.lstrip().startswith(("//", "#[path"))
                ]
                text = "\n".join(lines)
                if "CARGO_MANIFEST_DIR" not in text or not GOES_UP.search(text):
                    continue
                relative = str(path.relative_to(root))
                self.assertTrue(binaries, f"{relative} is in no test binary")
                for binary in binaries:
                    found += 1
                    with self.subTest(path=relative):
                        self.assertIn(binary, known)
        self.assertGreater(found, 20)

    def test_each_file_that_a_crate_includes_from_outside_selects_the_crate(self):
        root = Path(METADATA["workspace_root"])
        directories, _ = SELECTOR.workspace(METADATA)
        found = 0
        for crate, directory in sorted(directories.items()):
            crate_root = (root / directory).resolve()
            for path in sorted(crate_root.rglob("*.rs")):
                if "target" in path.relative_to(crate_root).parts:
                    continue
                for included in INCLUDE.findall(path.read_text()):
                    target = (path.parent / included).resolve()
                    if target.is_relative_to(crate_root):
                        continue
                    found += 1
                    changed = target.relative_to(root.resolve()).as_posix()
                    with self.subTest(crate=crate, file=changed):
                        selection = select(changed)
                        if selection["test-scope"] != "all":
                            self.assertIn(crate, json.loads(selection["test-crates"]))
        self.assertGreater(found, 0)

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
        patterns = {
            line.strip().removeprefix("- ").strip('"') for line in filters.splitlines()
        }
        for path in (
            "crates/provenance-store/**",
            "crates/provenance-core/**",
            "crates/provenance-macros/**",
            "crates/provenance-sdk/**",
            "crates/provenance-scanner/**",
            "crates/provenance-ste100/**",
        ):
            self.assertIn(path, patterns)
        # Every build input that selects the full suite can change what the
        # compiler prints.
        for path in SELECTOR.BUILD_INPUTS:
            with self.subTest(path=path):
                if path == ".github/workflows/ci.yml":
                    self.assertIn("*ci", patterns)
                elif path.endswith("/"):
                    self.assertIn(path + "**", patterns)
                else:
                    self.assertIn(path, patterns)
        job = self.job("compile-fail", "crap-quality")
        self.assertIn("needs.changes.outputs.compile-fail == 'true'", job)


if __name__ == "__main__":
    unittest.main()
