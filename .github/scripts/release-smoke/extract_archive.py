#!/usr/bin/env python3
import sys
import tarfile
import zipfile
from pathlib import Path, PurePosixPath


def validate_member(name: str) -> None:
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"archive member leaves the extraction directory: {name}")


def main() -> int:
    archive = Path(sys.argv[1])
    destination = Path(sys.argv[2])
    destination.mkdir(parents=True, exist_ok=True)

    if archive.name.endswith(".tar.gz"):
        with tarfile.open(archive, "r:gz") as contents:
            for member in contents.getmembers():
                validate_member(member.name)
            contents.extractall(destination, filter="data")
    elif archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as contents:
            for member in contents.infolist():
                validate_member(member.filename)
            contents.extractall(destination)
    else:
        raise ValueError(f"unsupported archive format: {archive.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
