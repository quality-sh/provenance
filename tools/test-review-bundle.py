"""Serve the pinned generic renderer and real reads from a standalone binary."""

import argparse
import json
import os
from pathlib import Path
import selectors
import shutil
import socket
import subprocess
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from review_assets import PIN, prepare


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="saved pinned archive; otherwise download with gh")
    parser.add_argument("--binary-output", type=Path, help="retain the validated standalone binary")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory(prefix="provenance-review-bundle-") as temp:
        work = Path(temp)
        assets = work / "assets"
        prepare(assets, args.archive)
        inventory = {"/" + path.relative_to(assets).as_posix(): path.read_bytes()
                     for path in assets.rglob("*") if path.is_file()}
        assert {"/index.html", "/review.js", "/review.css", "/build-info.json"} <= inventory.keys()
        assert b"mountReview" in inventory["/review.js"]
        assert b"<script" not in inventory["/index.html"]
        env = dict(os.environ, PROVENANCE_REVIEW_ASSETS_DIR=str(assets))
        subprocess.run(
            ["cargo", "build", "--locked", "-p", "provenance-cli", "--bin", "provenance"],
            cwd=root, env=env, check=True,
        )
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=root
        ))
        suffix = ".exe" if os.name == "nt" else ""
        binary = work / f"provenance{suffix}"
        shutil.copy2(Path(metadata["target_directory"]) / "debug" / binary.name, binary)
        shutil.rmtree(assets)
        repo = work / "repository"
        state = repo / ".provenance/state"
        state.mkdir(parents=True)
        (state / "manifest.json").write_text(json.dumps({
            "schema_version": 2, "disposition_actor_ids": [],
            "scopes": [{"id": "default", "path_prefix": "."}],
        }))
        # The copied executable has no Node executable or asset directory available.
        host = subprocess.Popen(
            [str(binary), "review", "--repo", str(repo), "--repository-id", "A", "--scope", "default"],
            cwd=work, env=dict(os.environ, PATH=str(work)), stdout=subprocess.PIPE,
        )
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(host.stdout, selectors.EVENT_READ)
                assert selector.select(timeout=15), "startup timed out"
            config = json.loads(host.stdout.readline())

            def request(path, data=None, auth=False):
                headers = {"Content-Type": "application/json"}
                if auth:
                    headers["Authorization"] = f"Bearer {config['bearer']}"
                req = urllib.request.Request(config["endpoint"] + path, data=data, headers=headers)
                try:
                    return urllib.request.urlopen(req, timeout=5)
                except urllib.error.HTTPError as error:
                    return error

            with request("/") as response:
                assert response.read() == inventory["/index.html"]
            for path, expected in inventory.items():
                with request(path) as response:
                    assert response.read() == expected, path
                    assert response.headers["Cache-Control"] == "no-store"
                    if path.endswith(".js"):
                        assert response.headers.get_content_type() == "text/javascript"
            with request("/metadata") as response:
                assert response.status == 200
            for target, status in [("A", 200), ("B", 404)]:
                body = json.dumps({"context": {"repository": target, "scope": "default"}, "request": None}).encode()
                with request("/v7/operations/list-threads", body, auth=True) as response:
                    assert response.status == status
            with request("/assets/missing.js") as response:
                assert response.status == 404
            with request("/review-config", auth=True) as response:
                runtime = json.loads(response.read())
                assert runtime["repositoryId"] == "A"
                assert runtime["scope"] == "default"
                assert "bearer" not in runtime
            assert config["url"] == config["endpoint"] + "/"
        finally:
            host.terminate()
            try:
                host.wait(timeout=10)
            except subprocess.TimeoutExpired:
                host.kill()
                host.wait()
                raise
        assert host.returncode == 0
        address = urllib.parse.urlparse(config["endpoint"])
        with socket.socket() as probe:
            assert probe.connect_ex((address.hostname, address.port)) != 0
        with socket.socket() as probe:
            # Match Tokio's listener policy when HTTP connections remain in TIME_WAIT.
            probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            probe.bind((address.hostname, address.port))
        if args.binary_output:
            shutil.copy2(binary, args.binary_output)
        print(f"Served {len(inventory)} verified assets and authorized reads without Node or asset files.")
        print(f"Source {PIN['commit']}; SHA-256 {PIN['sha256']}.")
        print("The generic HTML is an empty shell. Document bootstrap and the adapter remain in .3.")


if __name__ == "__main__":
    main()
