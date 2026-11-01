#!/usr/bin/env python3
"""Select the Rust tests that a pull request needs from its changed files.

A changed file in a crate selects that crate. The tests run for the selected
crates and for every workspace crate that depends on them, through normal,
build or dev dependencies, because those tests link the changed code. A
change to a file that all builds read selects the full suite. Events other
than pull_request always select the full suite.
"""

import argparse
import json
from pathlib import Path, PurePosixPath
import sys

# Files that every build reads. A change to one of them selects the full
# suite. The compile-fail path filter in ci.yml must name each of them too.
BUILD_INPUTS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
    ".cargo/",
    ".config/nextest.toml",
    ".github/workflows/ci.yml",
    ".github/actions/",
)

# Files that every CI job reads, and files that make the selection. A
# change to one of them selects the full suite.
FULL_SUITE = BUILD_INPUTS + (
    ".github/scripts/select_tests",
    ".github/scripts/ci_ok",
    ".github/scripts/crap_gate",
    ".github/scripts/fixtures/crap_gate/",
)

# Files outside crates/ that tests read from the repository root, and the
# crates whose tests read them. The first match applies.
ROOT_INPUTS = (
    (".provenance/", ("provenance-cli", "provenance-store")),
    (".agents/skills/", ("provenance-cli",)),
    (".claude/skills/", ("provenance-cli",)),
    ("skills/", ("provenance-cli",)),
    ("tools/skills/", ("provenance-cli",)),
    ("tools/review-host/", ("provenance-cli",)),
    ("tools/review-assets.json", ("provenance-cli",)),
    ("README.md", ("provenance-cli",)),
    ("LICENSE", ("provenance-cli",)),
    (".gitattributes", ("provenance-cli",)),
    (".config/crate-dependencies.json", ("provenance-cli",)),
    (".github/", ("provenance-cli",)),
    ("packages/provenance/package.json", ("provenance-cli",)),
    ("packages/provenance/scripts/package-engine.js", ("provenance-cli",)),
    ("packages/create-provenance/package.json", ("provenance-cli",)),
    (
        "packages/provenance/test/fixtures/packed-consumer/rule-bindings.ts.fixture",
        ("provenance-scanner",),
    ),
    ("fixtures/provenance-macros/", ("provenance-macros",)),
    # The generator writes the client source from the contract.
    ("contracts/operations/", ("provenance-http-client",)),
    ("tools/operation-codegen/", ("provenance-http-client",)),
)

# Files that no Rust test reads. Other jobs check them.
NOT_TEST_INPUTS = (
    "docs/",
    "examples/",
    "packages/",
    "tools/",
    "scripts/",
    ".beads/",
    ".container-use/",
    "AGENTS.md",
    "CONTEXT.md",
    "rustfmt.toml",
    ".gitignore",
    ".gitleaks.toml",
)

# Test binaries that read the source of crates that they do not depend on.
# A changed file that starts with the prefix and ends with the suffix also
# selects the test binary.
SOURCE_READERS = (
    ("", ".rs", "provenance-cli::cli_structure"),
    ("crates/", "/Cargo.toml", "provenance-cli::crate_dependencies"),
    # These read the metadata of every workspace crate.
    ("crates/", "/Cargo.toml", "provenance-cli::release_structure"),
    ("crates/", "/Cargo.toml", "provenance-cli::release_onboarding_structure"),
    ("crates/provenance-sdk/", "", "provenance-cli::cargo_nested_sdk"),
    ("crates/provenance-transport/src/", "", "provenance-contract::registration_ownership"),
    # This compares the bindings in the store crate with a text search of it.
    ("crates/provenance-store/", ".rs", "provenance-scanner::scanner_lexer_regressions"),
    *(
        (f"crates/{name}/src/", ".rs", "provenance-core::no_wildcard_gates")
        for name in (
            "provenance-core",
            "provenance-store",
            "provenance-cli",
            "provenance-scanner",
            "provenance-sdk",
        )
    ),
)


def workspace(metadata):
    """Return the crate directory and the workspace dependents of each crate."""
    root = PurePosixPath(Path(metadata["workspace_root"]).as_posix())
    names = {package["name"] for package in metadata["packages"]}
    directories = {}
    dependents = {name: set() for name in names}
    for package in metadata["packages"]:
        manifest = PurePosixPath(Path(package["manifest_path"]).as_posix())
        directories[package["name"]] = manifest.parent.relative_to(root).as_posix()
        for dependency in package["dependencies"]:
            if dependency["name"] in names and dependency.get("path"):
                dependents[dependency["name"]].add(package["name"])
    return directories, dependents


def reverse_closure(crates, dependents):
    closure = set()
    pending = list(crates)
    while pending:
        crate = pending.pop()
        if crate not in closure:
            closure.add(crate)
            pending.extend(dependents[crate])
    return closure


def changed_crates(files, directories):
    """Return the changed crates, or None when a file selects the full suite."""
    crates = set()
    owners = sorted(directories.items(), key=lambda item: -len(item[1]))
    for path in files:
        if path.startswith(FULL_SUITE):
            return None
        owner = next(
            (name for name, directory in owners if path.startswith(directory + "/")),
            None,
        )
        if owner is not None:
            crates.add(owner)
            continue
        if path.startswith("crates/"):
            return None
        mapped = next((found for entry, found in ROOT_INPUTS if path.startswith(entry)), None)
        if mapped is not None:
            crates.update(mapped)
            continue
        if path.startswith(NOT_TEST_INPUTS):
            continue
        return None
    return crates


def source_readers(files):
    return sorted(
        {
            binary
            for path in files
            for prefix, suffix, binary in SOURCE_READERS
            if path.startswith(prefix) and path.endswith(suffix)
        }
    )


def select(event, files, metadata):
    """Return the selection as a dictionary of workflow outputs."""
    directories, dependents = workspace(metadata)
    crates = None if event != "pull_request" else changed_crates(files, directories)
    if crates is None:
        return {
            "test-scope": "all",
            "test-crates": json.dumps(sorted(directories)),
            "test-packages": "--workspace",
            "test-filter": "all()",
            "doc-packages": "--workspace",
            "crate-dirs": "",
        }
    closure = reverse_closure(crates, dependents)
    readers = source_readers(files)
    built = closure | {binary.split("::", 1)[0] for binary in readers}
    if not built:
        return {
            "test-scope": "none",
            "test-crates": "[]",
            "test-packages": "",
            "test-filter": "none()",
            "doc-packages": "",
            "crate-dirs": "",
        }
    terms = [f"rdeps(={crate})" for crate in sorted(crates)]
    terms += [f"binary_id({binary})" for binary in readers]
    return {
        "test-scope": "some",
        "test-crates": json.dumps(sorted(closure)),
        "test-packages": " ".join(f"-p {crate}" for crate in sorted(built)),
        "test-filter": " | ".join(terms),
        "doc-packages": " ".join(f"-p {crate}" for crate in sorted(closure)),
        "crate-dirs": " ".join(directories[crate] for crate in sorted(closure)),
    }


def _arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--changed-files", type=Path, required=True)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main():
    args = _arguments()
    files = json.loads(args.changed_files.read_text())
    metadata = json.loads(args.metadata.read_text())
    outputs = select(args.event, files, metadata)
    lines = [f"{key}={value}" for key, value in outputs.items()]
    print("\n".join(lines))
    if args.output:
        with args.output.open("a") as output:
            output.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
