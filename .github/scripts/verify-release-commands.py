#!/usr/bin/env python3
"""Reject development commands in a release executable."""

import sys
from pathlib import Path

if b"dogfood" in Path(sys.argv[1]).read_bytes():
    sys.exit("release executable contains dogfood")
