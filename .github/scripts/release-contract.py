#!/usr/bin/env python3
import json
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"release target manifest: {message}")


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: release-contract.py <version> <target-manifest>")

    version = sys.argv[1]
    targets = json.loads(Path(sys.argv[2]).read_text())
    if not isinstance(targets, list) or not targets:
        fail("must contain a non-empty array")

    build = []
    smoke = []
    seen_targets = set()
    seen_packages = set()
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
        if entry["target"] in seen_targets:
            fail(f"duplicate target {entry['target']}")
        seen_targets.add(entry["target"])

        npm = entry["npm"]
        npm_required = {"name", "os", "cpu", "binary"}
        if not isinstance(npm, dict) or not npm_required.issubset(npm):
            fail(f"{entry['target']} has incomplete npm package data")
        if set(npm) - (npm_required | {"libc"}):
            fail(f"{entry['target']} has unknown npm package data")
        if npm["name"] in seen_packages:
            fail(f"duplicate npm package {npm['name']}")
        seen_packages.add(npm["name"])

        package = f"provenance-v{version}-{entry['target']}"
        common = {
            "target": entry["target"],
            "archive": entry["archive"],
            "package": package,
            "archive_name": f"{package}.{entry['archive']}",
            "executable_suffix": entry["executable_suffix"],
        }
        build.append(
            {
                **common,
                "os": entry["build_os"],
                "binary": f"provenance{entry['executable_suffix']}",
            }
        )
        smoke.append({**common, "os": entry["smoke_os"]})

    compact = (",", ":")
    print(f"version={version}")
    print(f"build-matrix={json.dumps({'include': build}, separators=compact)}")
    print(f"smoke-matrix={json.dumps({'include': smoke}, separators=compact)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
