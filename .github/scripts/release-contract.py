#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path


SAFE_COMPONENT = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$")
SAFE_RUNNER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
PACKAGE_MANIFEST = Path(__file__).resolve().parents[2] / "packages/provenance/package.json"


def fail(message: str) -> None:
    raise SystemExit(f"release target manifest: {message}")


def require_nonempty_string(value: object, location: str) -> str:
    if not isinstance(value, str) or not value or value != value.strip():
        fail(f"{location} must be a non-empty string without surrounding whitespace")
    if any(ord(character) < 32 or ord(character) == 127 for character in value):
        fail(f"{location} must not contain control characters")
    return value


def require_safe_component(value: object, location: str) -> str:
    text = require_nonempty_string(value, location)
    if not SAFE_COMPONENT.fullmatch(text):
        fail(f"{location} is not a safe path component: {text!r}")
    return text


def require_runner(value: object, location: str) -> str:
    runner = require_nonempty_string(value, location)
    if not SAFE_RUNNER.fullmatch(runner):
        fail(f"{location} must be a safe runner label")
    return runner


def expected_target_data(
    target: str,
) -> tuple[str, str, str, str, str, str | None, str]:
    match = re.fullmatch(r"(aarch64|x86_64)-apple-darwin", target)
    if match:
        cpu = "arm64" if match.group(1) == "aarch64" else "x64"
        return (
            "tar.gz",
            "",
            "macos",
            "darwin",
            cpu,
            None,
            f"@quality-sh/provenance-darwin-{cpu}",
        )

    match = re.fullmatch(r"(aarch64|x86_64)-pc-windows-msvc", target)
    if match:
        cpu = "arm64" if match.group(1) == "aarch64" else "x64"
        return (
            "zip",
            ".exe",
            "windows",
            "win32",
            cpu,
            None,
            f"@quality-sh/provenance-win32-{cpu}-msvc",
        )

    match = re.fullmatch(r"(aarch64|x86_64)-unknown-linux-gnu", target)
    if match:
        cpu = "arm64" if match.group(1) == "aarch64" else "x64"
        return (
            "tar.gz",
            "",
            "ubuntu",
            "linux",
            cpu,
            "glibc",
            f"@quality-sh/provenance-linux-{cpu}-gnu",
        )

    fail(f"unsupported Rust release target {target!r}")


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: release-contract.py <version> <target-manifest>")

    version = require_safe_component(sys.argv[1], "version")
    prerelease = "-" in version.split("+", 1)[0]
    try:
        targets = json.loads(Path(sys.argv[2]).read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(str(error))
    if not isinstance(targets, list) or not targets:
        fail("must contain a non-empty array")

    build = []
    smoke = []
    seen_targets = set()
    actual_packages = set()
    for entry in targets:
        required = {
            "build_os",
            "smoke_os",
            "target",
            "archive",
            "executable_suffix",
            "npm",
        }
        if not isinstance(entry, dict) or set(entry) != required:
            fail(f"each entry must contain exactly {sorted(required)}")
        target = entry["target"]
        if not isinstance(target, str):
            fail("target must be a string")
        if target in seen_targets:
            fail(f"duplicate target {target}")
        seen_targets.add(target)
        (
            expected_archive,
            expected_suffix,
            runner_family,
            expected_os,
            expected_cpu,
            expected_libc,
            expected_name,
        ) = expected_target_data(target)

        build_os = require_runner(entry["build_os"], "build_os")
        smoke_os = require_runner(entry["smoke_os"], "smoke_os")
        for field, runner in (("build_os", build_os), ("smoke_os", smoke_os)):
            if not runner.startswith(f"{runner_family}-"):
                fail(f"{target} {field} must use an {runner_family} runner")

        archive = entry["archive"]
        if archive != expected_archive:
            fail(f"{target} archive does not match target")
        executable_suffix = entry["executable_suffix"]
        if executable_suffix != expected_suffix:
            fail(f"{target} executable_suffix does not match target")

        npm = entry["npm"]
        expected_npm_fields = {"name", "os", "cpu"}
        if expected_libc is not None:
            expected_npm_fields.add("libc")
        if not isinstance(npm, dict) or set(npm) != expected_npm_fields:
            fail(f"{target} npm fields do not match target")
        npm_name = npm["name"]
        npm_os = npm["os"]
        npm_cpu = npm["cpu"]
        npm_libc = npm.get("libc")
        if npm_os != [expected_os]:
            fail(f"{target} npm os does not match target")
        if npm_cpu != [expected_cpu]:
            fail(f"{target} npm cpu does not match target")
        expected_libc_values = None if expected_libc is None else [expected_libc]
        if npm_libc != expected_libc_values:
            fail(f"{target} npm libc does not match target")
        if npm_name != expected_name:
            fail(f"{target} npm package name does not match target")
        actual_packages.add(npm_name)

        package = f"provenance-v{version}-{target}"
        common = {
            "target": target,
            "archive": archive,
            "package": package,
            "archive_name": f"{package}.{archive}",
            "executable_suffix": executable_suffix,
        }
        build.append(
            {
                **common,
                "os": build_os,
                "binary": f"provenance{executable_suffix}",
            }
        )
        smoke.append({**common, "os": smoke_os})

    try:
        package_manifest = json.loads(PACKAGE_MANIFEST.read_text())
        promised_packages = set(package_manifest["optionalDependencies"])
    except (OSError, json.JSONDecodeError, KeyError, TypeError) as error:
        fail(f"cannot read package optionalDependencies: {error}")
    if actual_packages != promised_packages:
        missing = sorted(promised_packages - actual_packages)
        extra = sorted(actual_packages - promised_packages)
        fail(
            "release targets do not match package optionalDependencies "
            f"(missing={missing!r}, extra={extra!r})"
        )

    compact = (",", ":")
    print(f"version={version}")
    print(f"npm-channel={'next' if prerelease else 'latest'}")
    print(f"prerelease={str(prerelease).lower()}")
    print(f"build-matrix={json.dumps({'include': build}, separators=compact)}")
    print(f"smoke-matrix={json.dumps({'include': smoke}, separators=compact)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
