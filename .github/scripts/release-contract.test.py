#!/usr/bin/env python3
import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("release-contract.py")
TARGETS = json.loads((Path(__file__).resolve().parents[1] / "release-targets.json").read_text())
RELEASE_WORKFLOW = Path(__file__).resolve().parents[1] / "workflows/release.yml"


def valid_target() -> dict:
    return {
        "build_os": "ubuntu-latest",
        "smoke_os": "ubuntu-latest",
        "target": "x86_64-unknown-linux-gnu",
        "archive": "tar.gz",
        "executable_suffix": "",
        "npm": {
            "name": "@quality-sh/provenance-linux-x64-gnu",
            "os": ["linux"],
            "cpu": ["x64"],
            "libc": ["glibc"],
        },
    }


def run_contract(targets: list, version: str = "0.2.2") -> subprocess.CompletedProcess:
    with tempfile.TemporaryDirectory() as temporary:
        manifest = Path(temporary) / "release-targets.json"
        manifest.write_text(json.dumps(targets))
        return subprocess.run(
            ["python3", str(SCRIPT), version, str(manifest)],
            check=False,
            capture_output=True,
            text=True,
        )


class ReleaseContractTests(unittest.TestCase):
    def test_generated_consumers_gate_every_publication_path_in_preflight(self) -> None:
        workflow = RELEASE_WORKFLOW.read_text()
        preflight = workflow.split("\n  build:", 1)[0]
        self.assertIn(
            "node packages/provenance/scripts/generate-release-consumers.js --check",
            preflight,
        )
        for job, next_job in (
            ("publish", "publish-crates"),
            ("publish-crates", "publish-npm"),
            ("publish-npm", "smoke"),
        ):
            with self.subTest(job=job):
                block = workflow.split(f"\n  {job}:", 1)[1].split(
                    f"\n  {next_job}:", 1
                )[0]
                self.assertIn("- preflight", block)

    def test_emits_safe_build_and_smoke_entries(self) -> None:
        result = run_contract(TARGETS)
        self.assertEqual(result.returncode, 0, result.stderr)
        outputs = dict(line.split("=", 1) for line in result.stdout.splitlines())
        build_entries = json.loads(outputs["build-matrix"])["include"]
        smoke_entries = json.loads(outputs["smoke-matrix"])["include"]
        build = build_entries[0]
        smoke = smoke_entries[0]
        self.assertEqual(outputs["version"], "0.2.2")
        self.assertEqual(outputs["npm-channel"], "latest")
        self.assertEqual(outputs["prerelease"], "false")
        self.assertEqual(len(build_entries), 4)
        self.assertEqual(len(smoke_entries), 4)
        self.assertEqual(build["binary"], "provenance")
        self.assertEqual(
            build["archive_name"],
            "provenance-v0.2.2-x86_64-unknown-linux-gnu.tar.gz",
        )
        self.assertEqual(smoke["os"], "ubuntu-latest")

    def test_build_metadata_hyphen_does_not_make_a_stable_version_prerelease(self) -> None:
        result = run_contract(TARGETS, "1.0.0+build-9")
        self.assertEqual(result.returncode, 0, result.stderr)
        outputs = dict(line.split("=", 1) for line in result.stdout.splitlines())
        self.assertEqual(outputs["npm-channel"], "latest")
        self.assertEqual(outputs["prerelease"], "false")

    def test_rejects_unsafe_or_malformed_target_data(self) -> None:
        cases = {
            "empty target": (("target",), "", "unsupported Rust release target"),
            "escaping target": (("target",), "../../escape", "unsupported Rust release target"),
            "numeric target": (("target",), 42, "target must be a string"),
            "wrong archive": (("archive",), "rar", "archive does not match target"),
            "empty build runner": (("build_os",), "", "build_os must be"),
            "numeric smoke runner": (("smoke_os",), 42, "smoke_os must be"),
            "control character in runner": (
                ("build_os",),
                "ubuntu-latest\u007f",
                "control characters",
            ),
            "malformed npm name": (
                ("npm", "name"),
                "@quality-sh/../../escape",
                "npm package name does not match target",
            ),
            "npm os is not an array": (("npm", "os"), "linux", "npm os does not match"),
            "empty npm cpu array": (("npm", "cpu"), [], "npm cpu does not match"),
            "numeric npm libc value": (("npm", "libc"), [1], "npm libc does not match"),
            "invented npm os": (("npm", "os"), ["plan9"], "npm os does not match"),
            "invented npm cpu": (("npm", "cpu"), ["mips"], "npm cpu does not match"),
            "supported but wrong npm cpu": (
                ("npm", "cpu"),
                ["arm64"],
                "does not match target",
            ),
            "invented npm libc": (("npm", "libc"), ["musl"], "npm libc does not match"),
            "mixed npm os": (("npm", "os"), ["linux", "win32"], "npm os does not match"),
            "duplicate npm cpu": (("npm", "cpu"), ["x64", "x64"], "npm cpu does not match"),
            "valid-looking wrong npm name": (
                ("npm", "name"),
                "@quality-sh/provenance-linux-x64-typo",
                "does not match target",
            ),
            "executable suffix on tar archive": (
                ("executable_suffix",),
                ".exe",
                "executable_suffix does not match target",
            ),
            "zip archive without executable suffix": (
                ("archive",),
                "zip",
                "archive does not match target",
            ),
        }
        for name, (path, value, expected_error) in cases.items():
            with self.subTest(name=name):
                target = copy.deepcopy(valid_target())
                current = target
                for component in path[:-1]:
                    current = current[component]
                current[path[-1]] = value
                result = run_contract([target])
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(expected_error, result.stderr)

    def test_rejects_windows_npm_with_unix_executable_suffix(self) -> None:
        target = copy.deepcopy(
            next(
                target
                for target in TARGETS
                if target["target"] == "x86_64-pc-windows-msvc"
            )
        )
        target["executable_suffix"] = ""
        result = run_contract([target])
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("executable_suffix does not match target", result.stderr)

    def test_rejects_manifest_missing_a_promised_target(self) -> None:
        result = run_contract(TARGETS[:-1])
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("optionalDependencies", result.stderr)

    def test_rejects_runners_from_the_wrong_os_family(self) -> None:
        for runner_field in ("build_os", "smoke_os"):
            with self.subTest(runner_field=runner_field):
                targets = copy.deepcopy(TARGETS)
                linux = next(
                    target
                    for target in targets
                    if target["target"] == "x86_64-unknown-linux-gnu"
                )
                linux[runner_field] = "macos-latest"
                result = run_contract(targets)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertIn(f"{runner_field} must use an ubuntu runner", result.stderr)

    def test_rejects_redundant_binary_and_unsafe_version_components(self) -> None:
        target = valid_target()
        target["npm"]["binary"] = "provenance"
        result = run_contract([target])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("npm fields do not match target", result.stderr)
        result = run_contract([valid_target()], "../0.2.2")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("safe path component", result.stderr)

    def test_rejects_duplicate_targets(self) -> None:
        first = valid_target()
        second = copy.deepcopy(first)
        result = run_contract([first, second])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate target", result.stderr)


if __name__ == "__main__":
    unittest.main()
