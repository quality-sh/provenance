#!/usr/bin/env python3
"""Require the release executable to name a clean build and to reject the
development command."""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="provenance-release-commands-") as directory:
    environment = {
        **os.environ,
        "NO_COLOR": "1",
        "PROVENANCE_DOGFOOD_DIR": str(Path(directory) / "notes"),
        "XDG_CONFIG_HOME": str(Path(directory) / "config"),
        "XDG_DATA_HOME": str(Path(directory) / "data"),
        "XDG_CACHE_HOME": str(Path(directory) / "cache"),
    }
    try:
        version = subprocess.run(
            [str(binary), "--version"],
            cwd=directory,
            env=environment,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        result = subprocess.run(
            [str(binary), "dogfood", "note", "--help"],
            cwd=directory,
            env=environment,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        sys.exit(f"Release command gate could not run the executable: {error}")

# A build from a checkout with changes to tracked build inputs names its commit
# with "-dirty". Such a binary is not the code of the release commit.
if version.returncode != 0 or "-dirty" in version.stdout:
    sys.stderr.write(version.stdout + version.stderr)
    sys.exit("Release command gate failed: the version does not name a clean build")

# Unknown root words select the record grammar. Require that parser to reject
# the development action. A hidden command still accepts its own help request.
expected = "error: invalid value 'note' for '[ACTION]'\n"
if result.returncode != 2 or result.stdout or not result.stderr.startswith(expected):
    sys.stderr.write(result.stdout + result.stderr)
    sys.exit("Release command gate failed: dogfood note was not rejected by the record parser")

print(f"Release command gate passed: {version.stdout.strip()} is clean and dogfood note is not a command")
