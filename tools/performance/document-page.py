#!/usr/bin/env python3
"""Create a 300-record document through the CLI, or time its first page.

Use a new directory for `seed`. Use the same directory and CI artifact binary
for `time` before and after the change. No time limit is checked in CI.
"""
import argparse
import json
import statistics
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("action", choices=["seed", "time"])
parser.add_argument("binary", type=Path)
parser.add_argument("repository", type=Path)
args = parser.parse_args()
binary = str(args.binary.resolve())
repository = str(args.repository.resolve())


def call(*command, body=None):
    command = [binary, *command, "--repo", repository, "--format", "json", "--quiet"]
    if body is not None:
        command.append("--stdin")
    result = subprocess.run(command, input=json.dumps(body) if body is not None else None,
                            text=True, capture_output=True, timeout=120, check=True)
    return json.loads(result.stdout)


def create(collection, **body):
    return call(collection, "create", body=body)["data"]


if args.action == "seed":
    args.repository.mkdir(parents=True, exist_ok=True)
    subprocess.run([binary, "init", "--path", repository, "--scope", "default",
                    "--path-prefix", ".", "--disposition-actor-id", "reviewer", "--quiet"],
                   check=True, timeout=120)
    create("requirements", id="req_document", statement="The system stores records.")
    for i in range(19):
        create("sources", id=f"source_{i:03}", name=f"Policy {i}", source_type="policy")
    for i in range(26):
        record = create("requirements", id=f"req_child_{i:03}",
                        statement="The system stores each record.", refines="req_document")
        if i < 6:
            decision = "accepted" if i < 3 else "rejected"
            body = dict(actor={"identity_type": "human", "id": "reviewer"}, decision=decision)
            if decision == "accepted":
                body["canonical_artifact"] = {"artifact_type": "requirement", "artifact_id": record["id"]}
            else:
                body.update(rationale="The text needs more detail.",
                            feedback={"role": "user", "body": "Specify the stored fields."})
            call("requirements", record["id"], "submissions",
                 record["decision"]["pending"]["proposal_id"], "decide", body=body)
        create("resolutions", id=f"res_{i:03}", title=f"Storage decision {i}",
               position="Use the record store.", rationale="The store keeps the records.",
               requirement_ids=[f"req_child_{i:03}"], status="draft",
               inputs=[dict(input_type="source_material", reference=f"source_{i % 19:03}",
                            summary="The policy describes the record store.")])
        create("topics", id=f"topic_{i:03}", requirement_id=f"req_child_{i:03}", title="Record fields")
        create("questions", id=f"question_{i:03}", topic_id=f"topic_{i:03}",
               question="Which fields does the record store?", resolution_method="research")
    for i in range(176):
        create("rules", id=f"rule_{i:03}", statement="The store keeps the record fields.",
               requirement_ids=[f"req_child_{i % 26:03}"], status="draft")
        if i % 25 == 0:
            print(f"Created {i + 1}/176 Rules", flush=True)
else:
    samples = []
    for _ in range(5):
        start = time.perf_counter()
        page = call("requirements", "req_document", "document", "get", "--limit", "50")
        samples.append(time.perf_counter() - start)
        assert len(page["entries"]) == 50, page
    print(json.dumps({"seconds": samples, "median": statistics.median(samples),
                      "review_totals": page["review_totals"]}, indent=2))
