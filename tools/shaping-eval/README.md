# Shaping evaluation

This harness compares shaping guidance with the real Provenance CLI. Evaluation
outputs stay outside the repository. The tests are implementation aids. They do
not bind Rules or approve product behaviour.

## Protocol

Run the harness on a POSIX host. Timeouts stop the runner process group.

Use the Codex CLI and Claude CLI. Never use OpenCode. Probe each runner before the
baseline: initialize an isolated Git repository, run one scripted turn, and have
the runner create a Requirement through the public CLI. Retain the command,
output, exit status, and model identity. Confirm that the runner can resume a
scripted session. If Claude cannot run, report the exact failure and retain an
explicit Claude coverage gap. Do not substitute another runner.

The configuration contains `runners`, with one entry per available runner:

```json
{
  "runners": [
    { "kind": "codex", "executable": "codex", "args": [], "model": "MODEL_ID" },
    { "kind": "claude", "executable": "claude", "args": [], "model": "MODEL_ID" }
  ]
}
```

Run the resume probe before the baseline:

```sh
node tools/shaping-eval/run.mjs --config /tmp/rpi/provenance-hlfa/runners.json --variant base --output /tmp/rpi/provenance-hlfa/eval/probe --probe true
```

This command makes a real Source write and reads that Source in a resumed turn.
Pin model IDs after the probe. The harness records the exact command for each
turn. Claude stream events also report the actual model. Codex model identity comes from the session `turn_context` metadata.
Selection is explicit in each command. A model mismatch stops the run. Retain model selection evidence from the runner,
rather than treating a model's prose claim as proof of its identity. Configure
`timeoutMs` if a runner needs more than the default ten minutes per turn. A
failed turn stops the evaluation; retain its output and report the failure.

Run from this repository:

```sh
node --test tools/shaping-eval/*.test.mjs
node tools/shaping-eval/run.mjs --config /tmp/rpi/provenance-hlfa/runners.json --variant base --output /tmp/rpi/provenance-hlfa/eval/base
node tools/shaping-eval/run.mjs --config /tmp/rpi/provenance-hlfa/runners.json --variant candidate --output /tmp/rpi/provenance-hlfa/eval/candidate
```

Each command runs three fresh sessions per runner. Each session starts in a new
temporary Git repository. `PROVENANCE_BIN` can select the candidate binary. The
harness initializes the graph and installs skills through the CLI. The base
uses source skills pinned to commit
`d981e0bd10f00900772bfc49544caa43d17f3154`. The candidate uses the current source
skills. The matching `docs/shaping.md` is also installed and hashed. These two skill files replace the fixture's installed copies; no
ownership stamp is copied. Other installed skills and the CLI must be identical
across variants. Retain the CLI build and environment with the results.

The initial brief and owner responses are in `fixtures/session.json`. Responses
arrive in their listed order through a resumed session. The runner must stop
after each response. The harness records public CLI list and review views after
each turn, and retains a final canonical snapshot. It does not change graph
records through private storage writes.

## Review rubric

Review the packets in `review/` without the variant mapping in `blind-key.json`.
Mix the base and candidate packets before review. A person must inspect every
record and the relevant transcript claims, not just the equality candidates.
Use the same rubric for both variants. A finding has `category`, `recordIds`,
`evidenceLocator`, and `reviewerAssessment: { isError, reason }`.

- `duplicate`: two linked records repeat the same obligation without adding
  meaning. Exact repeated text is detected automatically and requires a meaning
  assessment. Semantic repetition requires review. A precise obligation in a
  Resolution position is not automatically an error.
- `placement`: a record is attached above or outside the lowest Requirement
  that contains its obligation. Use the fixture's expected placement map.
  A spawned Requirement needs `refines` for its parent and `spawned_by` for its
  producing Resolution. These relationships have different meanings.
- `state`: a record or transcript claims acceptance without acceptance of the
  current content revision, or guesses owner authorship. Lifecycle alone is
  insufficient. The supplied active Rule with a proposed Resolution is grounded
  independently in the owner brief and is a negative control. Do not count it
  as an error because of that pair of statuses.
- `citationOnly`: a Resolution rationale gives citations or quotations without
  explaining the unresolved problem and why the choice is better than the
  alternatives. A citation in addition to an explanation is not an error.

Count a pair of repeated obligations once. Count each misplaced record once,
each unsupported acceptance or authorship claim once, and each deficient
rationale once. Retain evidence locators to graph records or transcript turns.
Also record non-errors when an equality candidate or negative control could be
misread. Mark all four categories in `completedCategories` only after a complete
review. Missing obligations, unscored packets, failed turns, and missing sessions
must be reported separately; they cannot become zero findings.

`totals(assessment)` validates finding evidence and returns category counts in
this order: duplicate, placement, state, citationOnly. Sum all three session
counts separately for each runner. Create base and candidate JSON files that map
runner names to those four totals, then run:

```sh
node tools/shaping-eval/score.mjs /tmp/base-totals.json /tmp/candidate-totals.json /tmp/comparison.json
```

Each nonzero baseline category must decrease for each runner. Each zero baseline
must remain zero. A missing runner fails the comparison. Compare equal completed
session counts with identical fixture and settings. A failed gate requires a
revision and a fresh full comparison. Do not select favourable reruns. Publish
retained results as secret Gists and cite exact revisions from Sources and Beads.

Publish result and review JSON files. Keep the imported dictionary in the local
workspace.
