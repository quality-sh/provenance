---
name: provenance-swarm-backtrace
description: Reverse-engineer candidate requirements from an existing codebase with a multi-agent swarm. Use when the user wants to extract, mine, backtrace, or reverse-engineer requirements or rules from existing code, bootstrap a Provenance graph from a legacy system, or asks "what must be true for this code to be correct". Lands every candidate as a proposal against a commit-pinned source, never as an active requirement or rule.
---

# Swarm backtrace

This skill does shaping in reverse. Agents partition an existing codebase and extract
candidates. A candidate states *what must be true for this code to be correct* and names
the function that already decides it. Agents then remove duplicate candidates but keep
**all** evidence sites, challenge every candidate, and land each one as a proposal. The
source of each proposal is the codebase, pinned to a commit. This file is the full
procedure.

## Ground rules

Before you call code redundant or delete tests, run
`provenance search --file <path>` on the file.

1. **Proposals only.** Write every proposal with `promotion_state: "proposed"`. The run
   never creates a Requirement, a Rule, a binding, or a disposition. Extracted claims
   describe *current behavior*; the code may be wrong, and that is half the point. Keep
   enough territory and evidence for the proposal to surface when later work makes it
   relevant. Do not ask a human to dispose of the whole run.
2. **Pin the commit.** Record the exact commit in the Source. If the target repository
   changes during the run, the run is against the pinned commit, not HEAD.
3. **Evidence discipline.** Every claim cites typed evidence. Code evidence has
   `file_path` and `line`, and the bare symbol name wherever the behavior lives in a named
   function or type. A line is where you looked; the symbol identifies a candidate primary
   implementation and survives the next refactor. Other evidence types need no file
   location. Mark speculation `unsupported` or `exploratory`. Rate uncertainty with a
   rationale.
4. **Only the orchestrator lands the run.** Subagents return structured findings and
   write nothing to `.provenance/state/`. The orchestrator removes duplicates, validates,
   and lands one run through the CLI.

## Where the files go

Use one work directory per run, outside the tracked tree of the target, or in a path that
Git ignores:

```text
.provenance-swarm/<run-id>/
  partitions.json      the partition manifest (stage 1)
  scratch/             single records for `provenance validate`, helper scripts
  run/                 the run directory that `land` reads (stage 5)
    extractors/<partition>.json
    refuters/<batch>.json
    merge/merged.json
```

`land` reads only the files of the run directory contract (stage 5). Keep scratch files
in `scratch/`, not in `run/`. Every `.json` file in `run/extractors/` and
`run/refuters/` must be a Contribution.

## Pipeline

Run stage 1 and the landing inline. Fan out stages 2 and 4 with the Agent tool: one
Agent call per partition or candidate batch, all in a single message so they run at the
same time. Stage 3 waits for every extractor.

### 1. Scout (inline)

- Confirm the Provenance manifest exists (`.provenance/state/manifest.json`); if not:
  `provenance init --path . --scope <scope> --path-prefix .`
- Pin the target: `git -C <target> rev-parse HEAD`.
- Create the codebase Source, commit-pinned:

  ```sh
  provenance source_codebase_<repo-slug>_<short-sha> create --type source --scope <scope> \
    --name "<repo> @ <short-sha>" \
    --source-type system_state \
    --reference "git:<repo>@<full-sha>" \
    --commit-pin "<full-sha>" \
    --format json
  ```

  `<repo-slug>` uses only lowercase letters, digits, `_`, and `-`.

  `system_state` is the type for observed system behavior. Use `project_artifact` only
  when you backtrace docs or specs, not running code.
- Partition the codebase by module or subsystem (directory tree, crate or package
  boundaries, `wc -l` for size). Write the partition manifest to `partitions.json`. You
  need it for the completeness check in stage 5:

  ```json
  [{"name": "auth", "paths": ["src/auth/"], "lines": 4200,
    "entry_points": ["src/auth/mod.rs:login"]}]
  ```

**Partition size:** one agent context per partition. A partition that an agent cannot
read in its context gives shallow, hedged candidates: split it. A partition of three
files hides cross-file behavior from the agent: merge it. Aim for one subsystem (auth,
billing, sync protocol) of about 2 to 10 thousand lines. Cut along dependency seams, not
by name. Shared core code that everything reads can be its own partition *and* background
reading for the others.

### 2. Extract (parallel, one agent per partition)

Each extractor reads its partition and returns candidates. Give each one:

- Its partition paths, the pinned commit, and the anchor question: *what must be true
  for this code to be correct?*
- **The requirement test: a requirement survives a rewrite of the code; an
  implementation detail does not.** "Sessions expire after 30 minutes of inactivity"
  survives; "session TTL is stored in Redis with key prefix `sess:`" does not.
- The output shape: JSON in the final message, no store writes. Per candidate: a
  statement; its shape (`requirement` or `rule`, below); the candidate primary
  implementation symbol for each rule candidate; `file:line` evidence sites (several when
  the behavior spans files); a confidence from 0.0 to 1.0 with a one-line rationale; open
  questions. A suspicion that the agent cannot ground in a line of code goes in a
  separate `speculation` list, never among the candidates.

**A good candidate statement** describes externally observable behavior or an
invariant, in domain language, testable without the source:
- Good: "A shift cannot be published without an assigned worker."
- Good: "Sync retries are capped at 5 with exponential backoff."
- Bad (code structure): "`ShiftPublisher` validates via `WorkerAssignmentGuard`."
- Bad (restated code): "The `MAX_RETRIES` constant is 5."

**Two shapes of candidate.** Every extractor returns both and keeps them apart:

- A **requirement candidate** is prose about what must be true. It survives a rewrite of
  the code and names no symbol, because no single symbol owns it: the behavior is spread
  across the partition, or nothing implements it cleanly. Most output has this shape.
- A **rule candidate** is an atomic behavioral obligation with evidence for a candidate
  primary implementation: one function, or one type whose construction makes the wrong
  state impossible to construct. It **names the implementation symbol** (file plus bare
  function or type name) with its statement. This workflow asks for that symbol as a
  conservative evidence threshold; the symbol does not define a Rule.

The statement stays in domain language in both shapes. "Sync retries are capped at 5
with exponential backoff" is the statement either way. A rule candidate also gives
`src/sync/retry.rs` + `retry_budget` as its candidate primary implementation.

**Never drop the symbol.** An extractor that finds a candidate primary implementation
and reports only prose loses the most expensive thing it found. The shaping loop can
land the Rule later, but it cannot recover a dropped symbol without another read of the
partition. If you cannot name one symbol, give a requirement candidate; do not guess a
function.

### 3. Remove duplicates and merge (waits for all extractors)

The same requirement arrives in different words from different partitions. Cluster by
meaning, not by words. For each cluster:

- Write one merged statement (the sharpest phrasing, or a new one). Run it through the
  `provenance-grounded-writing` skill's climbing test first; a merge is where
  capability-list language comes in.
- **Keep ALL evidence sites from every duplicate; never pick one winner.** Independent
  sites are the strongest sign that the behavior is intentional.
- Keep the confidence rationale with the most context. Record disagreement between
  extractors as a contested point for stage 4.
- When one duplicate names a candidate implementation symbol and the other only
  describes the behavior, keep the symbol and merge as a rule candidate. Stage 4 decides
  if the symbol is a sound primary implementation.
- Give each merged candidate a stable `proposal_key`: `backtrace/<partition>/<slug>`, or
  `backtrace/cross/<slug>` for a candidate merged across partitions. This key is the
  duplicate identity when the run is repeated.

### 4. Adversarial pass (parallel)

Fan out refuters over the merged candidates, about 10 to 20 candidates per agent. Give
each refuter the candidates *with* their evidence sites and read access to the code. Each
candidate gets three questions:

1. **Requirement or implementation detail?** Apply the rewrite test again, hostilely.
2. **Does the evidence support it?** Read every cited site again. A constant proves
   that a value exists, not that something enforces it. Find the enforcement path or
   demote the candidate.
3. **Is the primary implementation claim sound?** Rule candidates only. Open the named
   symbol and read it. Does that one function realize the whole obligation, or only a
   part, while a caller, a `WHERE` clause, or a schema constraint realizes the rest?
   Several implementation sites fail this workflow's binding threshold: demote the
   candidate to a requirement candidate and keep **every** site as evidence. The drift
   between those sites is the finding. Check the other direction too: a function that
   realizes three unrelated obligations needs three candidates or none. A type whose
   construction makes the wrong state impossible passes as the primary implementation.

Refuters return, per candidate, one verdict: uphold, demote-rule-to-requirement,
demote-to-detail, reclassify-as-speculation, or narrow-the-statement, with an objection
for each verdict that is not uphold. Refuters keep a demoted claim that can be true and
mark it `unsupported` or `exploratory`; they do not delete it.

### 5. Land (inline, serial)

Only the orchestrator lands. Do not send large JSON through shell flags. Write the run
directory files, validate them, then land the run directory.

Run directory contract (`land` reads these paths and no other files):

- `extractors/<partition>.json`: `<Contribution>` or `{"contribution": <Contribution>}`,
  one file for each extractor participant slot.
- `refuters/<batch>.json`: `<Contribution>` or `{"contribution": <Contribution>}`, one
  file for each refuter participant slot.
- `merge/merged.json`: `{"synthesis_packet": <SynthesisPacket>, "proposals":
  [<ProposalCard>, ...], "assertions": [<Assertion>, ...]}`. Every qualifying proposal
  has its assertion here. The run never writes dispositions.

Get the shapes from the CLI:

```sh
provenance schema show contribution --format json
provenance schema show synthesis-packet --format json
provenance schema show proposal --format json
provenance schema show assertion --format json
```

`provenance validate` accepts one bare record, not the wrapped form. A file that holds
`{"contribution": ...}` or `{"synthesis_packet": ..., "proposals": ...}` fails with
`unknown field`. To validate, write one bare record to `scratch/` and validate that file:

```sh
provenance validate contribution --input scratch/contribution.json --format json
provenance validate synthesis-packet --input scratch/synthesis.json --format json
provenance validate proposal --input scratch/proposal.json --format json
```

Then land:

```sh
provenance swarm-backtrace land --scope <scope> --run-dir .provenance-swarm/<run-id>/run --format json
```

`land` validates the records already in the scope together with the incoming records,
and writes all of them in one atomic write. A failure writes nothing. A follow-up run of
proposals only can use contributions and synthesis packets that are already landed, but
a qualifying proposal still needs its assertion.

Use `--replace` only to land regenerated contributions or synthesis packets again.
Proposal and assertion IDs are immutable: an identical record lands again with no change,
and a different record with the same ID fails. Without `--replace`, an existing
contribution or synthesis packet ID fails.

**Input rules that `land` enforces.** Each one stops the landing when it is not met:

- A proposal's `builds_on` lists assertion IDs of *other* proposals, never the
  proposal's own assertion and never a proposal ID. Omit `builds_on`, or write
  `"builds_on": []`, when the proposal builds on nothing. Its own assertion in `builds_on` fails with
  `proposal assertion lineage contains a cycle`.
- Each `reference_id` in a contribution's `evidence_references`, and each `claim_id` in
  its `material_claims`, occurs in one contribution only, across all files of the run and
  the contributions already in the scope. A refuter that cites the same code line as an
  extractor gives its reference a new ID, for example `evidence_auth_guard_refuter`.
  Proposal `traceability.evidence_references` can repeat contribution reference IDs.
- A `required_human_decisions` entry with `blocks_promotion: true`, or an
  `evidence_gaps` entry with `blocking_promotion: true`, stops the synthesis packet from
  asserting any proposal. An assertion against that packet fails with
  `assertion has a blocking human decision` or `assertion has a blocking evidence gap`.
  Set these flags
  honestly, and land the proposals of a blocked packet without assertions.

**Contribution shape.** Turn each extractor or refuter answer into one Contribution.
Each candidate becomes one material claim with its own evidence references:

```json
{
  "contribution": {
    "scope_id": "<scope>",
    "id": "contrib_backtrace_extract_auth",
    "target": {"artifact_type": "source", "artifact_id": "source_codebase_<repo-slug>_<short-sha>"},
    "participant_slot": "extract_auth",
    "stance": "support",
    "strongest_finding": "Publishing requires an assigned worker.",
    "evidence_references": [{"reference_id": "evidence_auth_guard", "evidence_type": "artifact",
      "summary": "src/auth.rs:publish_guard rejects a shift with no worker",
      "file_path": "src/auth.rs", "line": 12}],
    "material_claims": [{"claim_id": "claim_auth_guard",
      "statement": "A shift cannot be published without an assigned worker.",
      "evidence_type": "artifact", "evidence_reference_ids": ["evidence_auth_guard"],
      "confidence": 0.9}],
    "risks": [],
    "objections": [],
    "challenges": [],
    "suggested_artifact_changes": [],
    "unsupported_recommendations": [{"marker": "exploratory",
      "recommendation": "The guard may also apply to drafts."}],
    "uncertainty": {"level": "low", "rationale": "Direct guard evidence."},
    "open_questions": []
  }
}
```

- One contribution per participant slot (each extractor and each refuter).
- Extractor `stance` is usually `support`. Refuter `stance` is usually `oppose`, `mixed`,
  or `needs_more_evidence`. A refuter puts each verdict that is not uphold in
  `challenges` as `{"claim_id": ..., "objection": ...}`.
- Code evidence uses `evidence_type: "artifact"` with `file_path` and `line`. Its
  `summary` names the candidate primary implementation symbol when there is one.
- The extractor's `speculation` list goes in `unsupported_recommendations`, each with
  `marker` `"unsupported"` or `"exploratory"`.

Synthesis packet:

- One per run, with the codebase Source as its target.
- Keep consensus, contested claims, and minority objections separate; never average
  them.
- Put each candidate with no enforcement path in `evidence_gaps`.
- Put uncovered partitions (below) in `open_questions`.
- List each proposal in `suggested_artifacts` with the same `proposal_id`,
  `proposal_key`, and `proposal_type` as the proposal.

Proposals:

- One per merged candidate that survives stage 4.
- `proposal_type` is `"requirement_candidate"` for a behavioral requirement,
  `"rule_candidate"` for an atomic obligation with one evidenced candidate primary
  implementation function or type, `"source_gap"` for a missing policy or spec source,
  and `"question"` for a candidate that became only a question.
- A `rule_candidate` carries its binding where a human can act on it. An evidence
  reference has no symbol field (only `reference_id`, `evidence_type`, `summary`,
  `file_path`, `line`), so the implementation file goes in `file_path`, and the symbol
  goes in that reference's `summary` and in the proposal's `summary`:
  `<file>:<symbol> implements <what>`. A rule candidate whose symbol is only in an
  extractor's chat output is a requirement candidate with a better label.
- Set `confidence` (`0.0` to `1.0`) on the proposal; do not put the score in `summary`.
- Keep ALL merged evidence sites in `traceability.evidence_references` and the
  supporting claim IDs in `traceability.supporting_claim_ids`.
- Write `promotion_state: "proposed"` on every proposal.
- A proposal qualifies for an assertion when the synthesis packet lists it, its
  supporting claims are not contested, the claims have positive evidence (not
  `unsupported` or `exploratory`) owned by one contribution, and the packet has no
  blocking gap or decision. A qualifying proposal must have its assertion in
  `merge/merged.json`.

**What the graph shows after `land`.** A proposal that has an assertion reads as
`promotion_state: "asserted"` in `provenance api proposals`, although its row stays
`proposed`. A proposal with no assertion reads as `proposed`. Asserted is not accepted:
only a human disposition accepts, rejects, or defers a proposal, and the run writes
none.

**Completeness:** compare the landed run with `partitions.json`. Log each partition with
no extractor output (agent failed, context full, code unreadable); never skip it
silently. Add an open question to the synthesis packet ("partition X uncovered:
<reason>") and, if it matters, a `source_gap` proposal. A backtrace that looks complete
but dropped a partition is worse than one that says where it did not look.

### 6. Hand off to shaping

The backtrace output *feeds* the shaping loop; it does not finish anything. Tell the
human how many proposals landed, how many are asserted, what is contested, and what
surprised the refuters. Bring forward only a small set that is already contested or in
conflict and blocks a decision. The CLI has no standalone proposal-surface command. A
claim on a Topic returns the proposals in its explicit territory. For diff-driven work,
use `provenance api proposals` and look at the explicit evidence paths. Do not infer
territory from titles or graph proximity. Do not ask the human to dispose of the full
output, and do not create dispositions yourself.
