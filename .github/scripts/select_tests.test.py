#!/usr/bin/env python3
"""Tests for select_tests.py against the metadata of this workspace.

SELECT_TESTS_METADATA names a file with the output of
`cargo metadata --format-version 1 --no-deps`.
"""

import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import posixpath
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
    "provenance-scanner::method_word_conformance",
    "provenance-scanner::scanner_declarations",
    "provenance-store::review_cost",
)

# Files of library tests that read above their crate directory, with the same
# limit as ROOT_FILE_READERS. All the tests in src/ have one binary, the crate,
# so the list names each file. A new file of this kind must get a decision.
LIBRARY_ROOT_FILE_READERS = (
    # These read .provenance/state.
    "crates/provenance-store/src/operations/queries/tests/comparison/test_stores.rs",
    "crates/provenance-store/src/state_store/tests/legacy_coexistence.rs",
)

MANIFEST_DIR = "CARGO_MANIFEST_DIR"
# The ways to go up from CARGO_MANIFEST_DIR that the check finds: .parent(),
# .ancestors(), a string with a "../" part (also in concat!), and the string
# ".." (also through a constant).
GOES_UP = re.compile(r'\.parent\(\)|\bancestors\(\)|"[^"\n]*\.\./|"\.\."')
# The parts of Rust text that the checks read. The compiler reads the file at
# an include or a #[path] literal, so the checks resolve these paths.
RUST_TEXT = re.compile(
    r"""
    (?P<comment>//[^\n]*|/\*.*?\*/)
    | (?P<include>include_(?:str|bytes)!\(\s*
        (?:"(?P<file>[^"]+)"
        | concat!\(\s*env!\(\s*"CARGO_MANIFEST_DIR"\s*\)\s*,\s*"(?P<manifest>[^"]+)"\s*\)
        )\s*\))
    | (?P<path>\#\[path\s*=\s*"(?P<module>[^"]+)"\s*\])
    | (?P<inline>\bmod\s+(?P<name>\w+)\s*\{)
    | (?P<raw>\br(?P<hashes>\#*)".*?"(?P=hashes))
    | (?P<string>"(?:[^"\\]|\\.)*")
    | (?P<char>'(?:[^'\\]|\\.)')
    | (?P<open>\{)
    | (?P<close>\})
    """,
    re.VERBOSE | re.DOTALL,
)
ROOT_MODULES = ("lib.rs", "main.rs", "mod.rs", "build.rs")
CRATE_ROOT_DIRECTORIES = ("tests", "benches", "examples", "src/bin")


def module_directory(file, crate_directory, inline):
    """Return the directory that a #[path] literal in the file starts from."""
    relative = file.relative_to(crate_directory)
    root_module = file.name in ROOT_MODULES or relative.parent.as_posix() in CRATE_ROOT_DIRECTORIES
    if not inline:
        return file.parent
    start = file.parent if root_module else file.parent / file.stem
    return start.joinpath(*inline)


def compiler_reads(file, text, crate_directory):
    """Return (form, path) for each file that the compiler reads at a literal path.

    The paths are relative to the workspace root, as file and crate_directory are.
    """
    reads = []
    inline = []
    depth = 0
    for match in RUST_TEXT.finditer(text):
        kind = match.lastgroup
        if match.group("include"):
            if match.group("file"):
                reads.append(("include", file.parent / match.group("file")))
            else:
                path = match.group("manifest").lstrip("/")
                reads.append(("include with concat!", crate_directory / path))
        elif match.group("path"):
            directory = module_directory(file, crate_directory, [name for name, _ in inline])
            reads.append(("#[path]", directory / match.group("module")))
        elif match.group("inline"):
            inline.append((match.group("name"), depth))
            depth += 1
        elif kind == "open":
            depth += 1
        elif kind == "close":
            depth -= 1
            if inline and inline[-1][1] == depth:
                inline.pop()
    return [(form, PurePosixPath(posixpath.normpath(path))) for form, path in reads]


def goes_up_from_the_manifest_directory(text):
    """Tell if the text takes CARGO_MANIFEST_DIR and goes up from it.

    The check leaves out comments, and the include and #[path] literals that
    compiler_reads resolves. It cannot find these forms:
    - a path relative to the working directory, for example "../other/src";
    - Path::parent(path), the function-call form of .parent();
    - a path made at run time, for example from ".".repeat(2).
    The run on main runs every test, so a test of this kind still fails there.
    """

    def kept(match):
        if match.group("comment") or match.group("include") or match.group("path"):
            return ""
        return match.group(0)

    text = RUST_TEXT.sub(kept, text)
    return MANIFEST_DIR in text and GOES_UP.search(text) is not None


def above_crate_findings(crate, crate_directory, file, text, binaries, known):
    """Return a failure message for each read above the crate that no rule selects.

    crate_directory and file are relative to the workspace root. binaries are
    the test binaries that compile the file; a production file has none.
    known holds the test binaries and the library test files that a list names.
    """
    findings = []
    if binaries and goes_up_from_the_manifest_directory(text):
        for binary in binaries:
            if binary in known or file.as_posix() in known:
                continue
            library = "::" not in binary
            only_root = (
                f"add the path {file} to LIBRARY_ROOT_FILE_READERS in select_tests.test.py"
                if library
                else f"add {binary} to ROOT_FILE_READERS in select_tests.test.py"
            )
            findings.append(
                f"{file} goes up from CARGO_MANIFEST_DIR, so it can read files outside "
                f"{crate_directory}. Find the files that it reads. Then do one of these "
                f"steps. If it reads the Rust source of a crate that {crate} does not depend "
                f"on, add {binary} to RUST_SOURCE_READERS in select_tests.py. If it reads "
                f"other files of such a crate, add {binary} to SOURCE_READERS in "
                f"select_tests.py. If it reads only files that ROOT_INPUTS maps to {crate}, "
                f"or files of crates that {crate} depends on, {only_root}."
            )
    for form, target in compiler_reads(file, text, crate_directory):
        if target.is_relative_to(crate_directory):
            continue
        if target.parts[0] == "..":
            findings.append(
                f"{file} reads {target} ({form}), and this file is outside the repository. "
                "Move the file into the repository."
            )
            continue
        selection = select(target.as_posix())
        if selection["test-scope"] == "all" or crate in json.loads(selection["test-crates"]):
            continue
        if binaries and all(f"binary_id({b})" in selection["test-filter"] for b in binaries):
            continue
        findings.append(
            f"{file} reads {target} ({form}), but a change to {target} does not select "
            f"{crate}. If {target} is in a crate, add an entry to SOURCE_READERS in "
            f"select_tests.py that selects {', '.join(binaries) or crate}. If not, add an "
            f"entry to ROOT_INPUTS in select_tests.py that selects {crate}."
        )
    return findings


def binaries_of_test_files(crate, crate_directory):
    """Return the test binaries that compile each test file of a crate.

    A file in src/ belongs to the library tests, which have the crate name as
    their binary id. Production files in src/ are left out. A file in src/
    counts as a test file when its name contains "test", so a name such as
    latest.rs also counts; this makes the check stricter and is safe.
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


def glob_matches(pattern, path):
    """Tell if a path filter pattern of ci.yml matches the path.

    This is the part of the picomatch syntax that ci.yml uses: "**/", "**"
    and "*". dorny/paths-filter matches dot files too.
    """
    expression = ""
    at = 0
    while at < len(pattern):
        if pattern.startswith("**/", at):
            expression += "(?:.*/)?"
            at += 3
        elif pattern.startswith("**", at):
            expression += ".*"
            at += 2
        elif pattern[at] == "*":
            expression += "[^/]*"
            at += 1
        else:
            expression += re.escape(pattern[at])
            at += 1
    return re.fullmatch(expression, path) is not None


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
            "-p provenance-cli -p provenance-core -p provenance-store",
        )
        self.assertEqual(
            selection["test-filter"],
            " | ".join(
                ["rdeps(=provenance-cli)"]
                + [f"binary_id({binary})" for binary in SELECTOR.RUST_SOURCE_READERS]
            ),
        )

    def test_a_rust_change_in_any_crate_selects_every_rust_source_reader(self):
        # A change to a test file of one crate can break a test of a different
        # crate that reads every Rust file. A direct Git start in a CLI test
        # file is an example: the store test that reads every crate finds it.
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

    def test_an_ignore_file_runs_the_full_suite(self):
        # The scanner obeys these files, and tests scan the workspace with it.
        for path in (
            ".gitignore",
            "examples/rust-sdk/.gitignore",
            "crates/provenance-store/.gitignore",
            "crates/provenance-store/src/.gitignore",
        ):
            with self.subTest(path=path):
                self.assert_full_suite(select(path))

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
        for file in LIBRARY_ROOT_FILE_READERS:
            self.assertTrue((root / file).is_file(), file)

    def test_each_read_above_a_crate_is_selected(self):
        # A test that reads the files of another crate must run when the
        # files change. A new test of this kind fails here until a list names
        # it, so the selection cannot leave it out without a decision. The
        # check reads every Rust file of each crate for the include and
        # #[path] literals, and each test file for the other forms.
        root = Path(METADATA["workspace_root"])
        directories, _ = SELECTOR.workspace(METADATA)
        known = set(SELECTOR.RUST_SOURCE_READERS) | set(ROOT_FILE_READERS)
        known |= {binary for _, _, binary in SELECTOR.SOURCE_READERS}
        known |= set(LIBRARY_ROOT_FILE_READERS)
        tests_found = 0
        literals_found = 0
        for crate, directory in sorted(directories.items()):
            crate_directory = PurePosixPath(directory)
            test_files = binaries_of_test_files(crate, root / directory)
            for path in sorted((root / directory).rglob("*.rs")):
                if "target" in path.relative_to(root / directory).parts:
                    continue
                file = PurePosixPath(path.relative_to(root).as_posix())
                text = path.read_text()
                binaries = test_files.get(path, [])
                if path in test_files and goes_up_from_the_manifest_directory(text):
                    self.assertTrue(
                        binaries,
                        f"{file} goes up from CARGO_MANIFEST_DIR, but no test binary names "
                        "its module. Add a mod declaration for it, or remove the file.",
                    )
                    tests_found += 1
                literals_found += sum(
                    not target.is_relative_to(crate_directory)
                    for _, target in compiler_reads(file, text, crate_directory)
                )
                findings = above_crate_findings(
                    crate, crate_directory, file, text, binaries, known
                )
                with self.subTest(path=str(file)):
                    if findings:
                        self.fail("\n".join(findings))
        # The floors make sure that the patterns still find the tests and the
        # literals that are in the workspace today (27 test files and 60 literals
        # when this check was written). A pattern that finds nothing would let
        # every check pass.
        self.assertGreater(tests_found, 20, "the check found too few tests that go up")
        self.assertGreater(literals_found, 0, "the check found no include or #[path] literal")

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


CORE = PurePosixPath("crates/provenance-core")


def core_test_findings(text, name="reads_the_cli"):
    """Return the findings for a test file of provenance-core that has the text.

    provenance-core does not depend on provenance-cli, so a read of the CLI
    files is a finding unless a list names the test.
    """
    file = CORE / "tests" / f"{name}.rs"
    known = set(SELECTOR.RUST_SOURCE_READERS) | set(ROOT_FILE_READERS)
    return above_crate_findings(
        "provenance-core", CORE, file, text, [f"provenance-core::{name}"], known
    )


class AboveCrateCheckTests(unittest.TestCase):
    def test_the_check_finds_each_form_that_goes_up(self):
        for form, text in (
            ("join of ..", 'Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")'),
            ("parent", 'Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()'),
            ("ancestors", 'Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2)'),
            (
                "concat! with ../",
                'read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../provenance-cli/src"))',
            ),
            (
                ".. through a constant",
                'const UP: &str = "..";\n'
                'fn f() { Path::new(env!("CARGO_MANIFEST_DIR")).join(UP).join(UP); }',
            ),
        ):
            with self.subTest(form=form):
                findings = core_test_findings(text)
                self.assertEqual(len(findings), 1, findings)
                self.assertIn(
                    "add provenance-core::reads_the_cli to RUST_SOURCE_READERS", findings[0]
                )

    def test_the_check_finds_each_include_and_path_literal_that_leaves_the_crate(self):
        cli_lib = '"/../provenance-cli/src/lib.rs"'
        for form, text in (
            ("include_str! with ..", 'include_str!("../../provenance-cli/src/lib.rs")'),
            ("include_bytes! with ..", 'include_bytes!("../../provenance-cli/src/lib.rs")'),
            (
                "include_str! with concat!",
                f'include_str!(concat!(env!("CARGO_MANIFEST_DIR"), {cli_lib}))',
            ),
            (
                "include_bytes! with concat!",
                f'include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), {cli_lib}))',
            ),
            (
                "#[path]",
                '#[path = "../../provenance-cli/tests/support/mod.rs"]\nmod support;',
            ),
            (
                "#[path] in an inline module",
                'mod support {\n    #[path = "../../../provenance-cli/tests/support/mod.rs"]\n'
                "    mod shared;\n}",
            ),
        ):
            with self.subTest(form=form):
                findings = core_test_findings(text)
                self.assertEqual(len(findings), 1, findings)
                self.assertIn("crates/provenance-cli/", findings[0])
                self.assertIn("does not select provenance-core", findings[0])

    def test_a_path_literal_into_a_crate_that_the_test_depends_on_is_no_finding(self):
        # The mcp tests use the support files of the transport crate. The mcp
        # crate depends on it, so a change to them selects the mcp tests.
        mcp = PurePosixPath("crates/provenance-mcp")
        file = mcp / "tests/resources.rs"
        text = (
            "mod support {\n"
            '    #[path = "../../../provenance-transport/tests/support/records.rs"]\n'
            "    pub mod records;\n}\n"
        )
        self.assertEqual(
            compiler_reads(file, text, mcp),
            [("#[path]", PurePosixPath("crates/provenance-transport/tests/support/records.rs"))],
        )
        findings = above_crate_findings(
            "provenance-mcp", mcp, file, text, ["provenance-mcp::resources"], set()
        )
        self.assertEqual(findings, [])

    def test_the_check_cannot_find_the_forms_that_its_doc_comment_names(self):
        # These tests read the CLI source, but the check does not find them.
        # The run on main runs every test, so such a test still fails there.
        for form, text in (
            ("relative to the working directory", 'read_dir("../provenance-cli/src")'),
            (
                "Path::parent",
                'Path::parent(Path::new(env!("CARGO_MANIFEST_DIR")))'
                '.unwrap().join("provenance-cli")',
            ),
            (
                "made at run time",
                'Path::new(env!("CARGO_MANIFEST_DIR"))'
                '.join(".".repeat(2)).join("provenance-cli")',
            ),
        ):
            with self.subTest(form=form):
                self.assertEqual(core_test_findings(text), [])

    def test_a_named_test_and_a_named_library_file_are_no_finding(self):
        text = 'Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")'
        self.assertEqual(core_test_findings(text, "no_wildcard_gates"), [])
        store = PurePosixPath("crates/provenance-store")
        named = PurePosixPath(LIBRARY_ROOT_FILE_READERS[0])
        other = store / "src/state_store/tests/new_reader.rs"
        known = set(LIBRARY_ROOT_FILE_READERS)
        findings = above_crate_findings(
            "provenance-store", store, named, text, ["provenance-store"], known
        )
        self.assertEqual(findings, [])
        findings = above_crate_findings(
            "provenance-store", store, other, text, ["provenance-store"], known
        )
        self.assertEqual(len(findings), 1, findings)
        self.assertIn(f"add the path {other} to LIBRARY_ROOT_FILE_READERS", findings[0])


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

    def test_the_test_job_runs_for_each_rust_file_and_each_ignore_file(self):
        # The Test job runs on a pull request only when the rust or contract
        # path filter matches. A change to a tracked .rs file selects tests,
        # and a change to a .gitignore file selects every test, so the rust
        # filter must match each of them.
        block = self.workflow.split("\n            rust:\n", 1)[1]
        block = block.split("\n            contract:", 1)[0]
        patterns = [
            line.strip().removeprefix("- ").strip('"')
            for line in block.splitlines()
            if line.strip().startswith('- "')
        ]
        tracked = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=WORKFLOW.parents[2],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.split("\0")
        paths = [
            path
            for path in tracked
            if path.endswith(".rs") or PurePosixPath(path).name == ".gitignore"
        ]
        self.assertGreater(len(paths), 100, "git ls-files found too few files")
        paths += ["tools/bench/new.rs", "new.rs", ".github/new/.gitignore"]
        for path in paths:
            with self.subTest(path=path):
                self.assertTrue(
                    any(glob_matches(pattern, path) for pattern in patterns),
                    f"The rust path filter in ci.yml does not match {path}. Add a pattern "
                    "for it, so that the Test job runs when it changes.",
                )
                self.assertNotEqual(select(path)["test-scope"], "none")

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
