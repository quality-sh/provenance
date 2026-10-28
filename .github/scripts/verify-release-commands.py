#!/usr/bin/env python3
"""Require the release executable to reject the development command."""

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

# Unknown root words select the record grammar. Require that parser to reject
# the development action. A hidden command still accepts its own help request.
expected = "error: invalid value 'note' for '[ACTION]'\n"
if result.returncode != 2 or result.stdout or not result.stderr.startswith(expected):
    sys.stderr.write(result.stdout + result.stderr)
    sys.exit("Release command gate failed: dogfood note was not rejected by the record parser")

print("Release command gate passed: dogfood note is not a command")
