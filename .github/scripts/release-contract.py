#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path


SAFE_COMPONENT = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$")
SAFE_RUNNER = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
NPM_PACKAGE = re.compile(r"^@quality-sh/provenance-[a-z0-9][a-z0-9-]*$")
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
    if not SAFE_COMPONENT.fullmatch(text) or text in {".", ".."}:
        fail(f"{location} is not a safe path component: {text!r}")
    return text


def require_runner(value: object, location: str) -> str:
    runner = require_nonempty_string(value, location)
    if not SAFE_RUNNER.fullmatch(runner):
        fail(f"{location} must be a safe runner label")
    return runner


def require_string_array(value: object, location: str) -> list[str]:
    if not isinstance(value, list) or not value:
        fail(f"{location} must be a non-empty array")
    items = [require_safe_component(item, location) for item in value]
    if len(items) != len(set(items)):
        fail(f"{location} must not contain duplicates")
    return items


def require_platform_array(
    value: object, location: str, supported: set[str]
) -> list[str]:
    items = require_string_array(value, location)
    if len(items) != 1:
        fail(f"{location} must contain exactly one value")
    if items[0] not in supported:
        fail(f"unsupported {location} value {items[0]!r}")
    return items


def expected_npm_data(target: str) -> tuple[str, str, str | None, str]:
    match = re.fullmatch(r"(aarch64|x86_64)-apple-darwin", target)
    if match:
        cpu = "arm64" if match.group(1) == "aarch64" else "x64"
        return "darwin", cpu, None, f"@quality-sh/provenance-darwin-{cpu}"

    match = re.fullmatch(r"(aarch64|x86_64)-pc-windows-msvc", target)
    if match:
        cpu = "arm64" if match.group(1) == "aarch64" else "x64"
        return "win32", cpu, None, f"@quality-sh/provenance-win32-{cpu}-msvc"

    match = re.fullmatch(r"(aarch64|x86_64)-unknown-linux-gnu", target)
    if match:
        cpu = "arm64" if match.group(1) == "aarch64" else "x64"
        return "linux", cpu, "glibc", f"@quality-sh/provenance-linux-{cpu}-gnu"

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
        build_os = require_runner(entry["build_os"], "build_os")
        smoke_os = require_runner(entry["smoke_os"], "smoke_os")
        target = require_safe_component(entry["target"], "target")
        archive = require_nonempty_string(entry["archive"], f"{target} archive")
        if archive not in {"tar.gz", "zip"}:
            fail(f"{target} uses unsupported archive format {archive!r}")
        executable_suffix = entry["executable_suffix"]
        expected_suffix = ".exe" if archive == "zip" else ""
        if executable_suffix != expected_suffix:
            fail(
                f"{target} archive {archive!r} requires executable_suffix "
                f"{expected_suffix!r}"
            )
        if target in seen_targets:
            fail(f"duplicate target {target}")
        seen_targets.add(target)

        npm = entry["npm"]
        npm_required = {"name", "os", "cpu"}
        if not isinstance(npm, dict) or not npm_required.issubset(npm):
            fail(f"{target} has incomplete npm package data")
        if set(npm) - (npm_required | {"libc"}):
            fail(f"{target} has unknown npm package data")
        npm_name = require_nonempty_string(npm["name"], f"{target} npm name")
        if not NPM_PACKAGE.fullmatch(npm_name):
            fail(f"{target} has invalid npm package name {npm_name!r}")
        npm_os = require_platform_array(
            npm["os"], "npm os", {"darwin", "linux", "win32"}
        )
        npm_cpu = require_platform_array(npm["cpu"], "npm cpu", {"arm64", "x64"})
        npm_libc = None
        if "libc" in npm:
            npm_libc = require_platform_array(npm["libc"], "npm libc", {"glibc"})
        if (npm_os == ["linux"]) != (npm_libc == ["glibc"]):
            fail(f"{target} npm os and libc are inconsistent")
        if (npm_os == ["win32"]) != (executable_suffix == ".exe"):
            fail(f"{target} npm os and executable suffix are inconsistent")
        expected_os, expected_cpu, expected_libc, expected_name = expected_npm_data(target)
        actual_platform = (
            npm_os[0],
            npm_cpu[0],
            None if npm_libc is None else npm_libc[0],
        )
        expected_platform = (expected_os, expected_cpu, expected_libc)
        if actual_platform != expected_platform:
            fail(f"{target} npm platform does not match target")
        if npm_name != expected_name:
            fail(f"{target} npm package name does not match target")
        actual_packages.add(npm_name)

        expected_archive = "zip" if expected_os == "win32" else "tar.gz"
        if archive != expected_archive:
            fail(f"{target} must use the {expected_archive!r} archive format")
        runner_family = {"darwin": "macos", "linux": "ubuntu", "win32": "windows"}[
            expected_os
        ]
        for field, runner in (("build_os", build_os), ("smoke_os", smoke_os)):
            if not runner.startswith(f"{runner_family}-"):
                fail(f"{target} {field} must use an {runner_family} runner")

        package = f"provenance-v{version}-{target}"
        require_safe_component(package, f"{target} archive package")
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
