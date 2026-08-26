#!/usr/bin/env python3
import json
import sys
from pathlib import Path

import yaml


def main() -> int:
    root = Path(__file__).resolve().parents[3]
    manifest = json.loads((root / ".github/release-targets.json").read_text())
    workflow = yaml.safe_load((root / ".github/workflows/release.yml").read_text())
    release_targets = workflow["jobs"]["build"]["strategy"]["matrix"]["include"]
    expected = [
        {"os": target["build_os"], "target": target["target"], "archive": target["archive"]}
        for target in manifest
    ]
    if release_targets != expected:
        print("release target matrix differs from .github/release-targets.json", file=sys.stderr)
        print(f"manifest: {expected!r}", file=sys.stderr)
        print(f"release:  {release_targets!r}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
