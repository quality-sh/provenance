"""Prepare the pinned, generic review renderer as a Cargo build input."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile


PIN = json.loads(Path(__file__).with_name("review-assets.json").read_text())


def extract_verified(archive, output, digest):
    data = Path(archive).read_bytes()
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError("review archive checksum mismatch")
    output = Path(output)
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as stream:
        members = stream.getmembers()
        for member in members:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or "\\" in member.name or ":" in member.name
                    or not (member.isfile() or member.isdir())):
                raise ValueError("review archive contains an unsafe entry")
        output.mkdir(parents=True, exist_ok=False)
        for member in members:
            path = output / member.name
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                with stream.extractfile(member) as source, path.open("wb") as target:
                    shutil.copyfileobj(source, target)


def prepare(output, archive=None):
    if archive is not None:
        extract_verified(archive, output, PIN["sha256"])
        return
    with tempfile.TemporaryDirectory(prefix="provenance-review-download-") as temp:
        subprocess.run([
            "gh", "run", "download", str(PIN["run"]), "--repo", PIN["repository"],
            "--name", PIN["artifact"], "--dir", temp,
        ], check=True)
        extract_verified(Path(temp) / PIN["archive"], output, PIN["sha256"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new directory outside source control")
    parser.add_argument("--archive", type=Path, help="use a saved archive without GitHub access")
    args = parser.parse_args()
    prepare(args.output, args.archive)
    print(f"PROVENANCE_REVIEW_ASSETS_DIR={args.output.resolve()}")
    print(f"SHA-256 {PIN['sha256']}; source {PIN['commit']}")
