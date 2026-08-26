#!/usr/bin/env python3
import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("release-contract.py")


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
    def test_emits_safe_build_and_smoke_entries(self) -> None:
        result = run_contract([valid_target()])
        self.assertEqual(result.returncode, 0, result.stderr)
        outputs = dict(line.split("=", 1) for line in result.stdout.splitlines())
        build = json.loads(outputs["build-matrix"])["include"][0]
        smoke = json.loads(outputs["smoke-matrix"])["include"][0]
        self.assertEqual(outputs["version"], "0.2.2")
        self.assertEqual(build["binary"], "provenance")
        self.assertEqual(
            build["archive_name"],
            "provenance-v0.2.2-x86_64-unknown-linux-gnu.tar.gz",
        )
        self.assertEqual(smoke["os"], "ubuntu-latest")

    def test_rejects_unsafe_or_malformed_target_data(self) -> None:
        cases = {
            "empty target": (("target",), "", "target must be"),
            "escaping target": (("target",), "../../escape", "safe path component"),
            "unsupported archive": (("archive",), "rar", "unsupported archive"),
            "empty build runner": (("build_os",), "", "build_os must be"),
            "numeric smoke runner": (("smoke_os",), 42, "smoke_os must be"),
            "control character in runner": (
                ("build_os",),
                "ubuntu-latest\u007f",
                "safe runner label",
            ),
            "malformed npm name": (
                ("npm", "name"),
                "@quality-sh/../../escape",
                "invalid npm package name",
            ),
            "npm os is not an array": (("npm", "os"), "linux", "non-empty array"),
            "empty npm cpu array": (("npm", "cpu"), [], "non-empty array"),
            "numeric npm libc value": (("npm", "libc"), [1], "non-empty string"),
            "invented npm os": (("npm", "os"), ["plan9"], "unsupported npm os"),
            "invented npm cpu": (("npm", "cpu"), ["mips"], "unsupported npm cpu"),
            "supported but wrong npm cpu": (
                ("npm", "cpu"),
                ["arm64"],
                "does not match target",
            ),
            "invented npm libc": (("npm", "libc"), ["musl"], "unsupported npm libc"),
            "mixed npm os": (("npm", "os"), ["linux", "win32"], "exactly one"),
            "duplicate npm cpu": (("npm", "cpu"), ["x64", "x64"], "duplicates"),
            "valid-looking wrong npm name": (
                ("npm", "name"),
                "@quality-sh/provenance-linux-x64-typo",
                "does not match target",
            ),
            "executable suffix on tar archive": (
                ("executable_suffix",),
                ".exe",
                "requires executable_suffix",
            ),
            "zip archive without executable suffix": (
                ("archive",),
                "zip",
                "requires executable_suffix",
            ),
            "windows npm with unix suffix": (
                ("npm", "os"),
                ["win32"],
                "inconsistent",
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

    def test_rejects_redundant_binary_and_unsafe_version_components(self) -> None:
        target = valid_target()
        target["npm"]["binary"] = "provenance"
        result = run_contract([target])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown npm package data", result.stderr)
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
