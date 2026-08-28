---
name: provenance-shaping
description: Guide turn-based requirement shaping in Provenance. Use when a user brings a loose idea, asks to refine requirements, work through open shaping questions, graduate fog, or run the Chart/Work loop against an anchor requirement. Land every resolved decision immediately into the graph.
---

# Shaping

Turn-based requirement definition for Provenance. Canonical design: `docs/shaping.md`;
where this skill diverges, that document wins.

The core discipline is **LAND-AS-YOU-GO**: every resolved decision is written to the
graph before you move on. The graph holds state between turns, not the conversation.

## Invariants

1. **Land every decision as it resolves.** Never carry a resolved-but-unrecorded decision
   in conversation state.
2. **Do not outrun context.** Stop before the map is too large to hand off accurately.
3. **Do not proceed past unratified decisions.** If the human has not accepted the
   position, it is a proposal, open question, or blocked-on-human fork, not a decision.
4. **Leave the map consistent at handoff.** Claims, fog, questions, and frontier must tell
   the next session what to do without reconstructing your chat.

One question per session is **not** the invariant. Grill turns can drain a topic question
by question; expensive methods claim one question and often stop at a phase boundary.

## Start every session

1. Confirm the repo has a Provenance store. If absent, ask before initializing unless the
   user already asked you to set it up:

   ```sh
   provenance init --path . --scope <scope> --path-prefix .
   ```

2. Load the low-resolution map:

   ```sh
   provenance prime --scope <scope> --format json
   provenance graph <anchor_requirement_id> --scope <scope> --format json
   provenance requirements fog show --scope <scope> --requirement-id <anchor_requirement_id> --format json
   provenance topics list --scope <scope> --format json
   provenance questions list --scope <scope> --format json
   provenance boundaries list --scope <scope> --format json
   ```

3. Treat the map as an index, not a store. Keep only gists in your working context and
   zoom into the available graph and rule views with `provenance graph <requirement_id>`
   and `provenance traceability <rule_id>`; use the shaping list commands above for their
   full records. Refer to artifacts by meaningful names plus ids; never hand off bare ids.

## Mode 1: Chart

Use Chart when the user brings a loose idea: "I want provenance docs shareable via a
short-lived link - how would that work?" Charting is one session's work. Do **not** also
resolve the questions you create.

### Chart workflow

1. **Create or select the anchor requirement.** Any requirement at any depth can anchor a
   shaping effort.

   ```sh
   provenance requirements create --scope <scope> \
     --id req_<stable_slug> \
     --statement "<feature-sized idea as a requirement>" \
     --status discovery \
     --format json
   ```

2. **Attach known sources to the anchor.** Requirements with neither a valid embedded
   source reference nor a valid `references` edge remain on the missing-source frontier,
   so add source references when evidence already exists.

   ```sh
   provenance requirements source-ref add --scope <scope> \
     --requirement-id <anchor_requirement_id> \
     --source-id <source_id> \
     --clause "<clause or section>" \
     --format json
   ```

3. **Grill only enough to map the effort.** Surface appetite, no-gos, fog, and the first
   sharp questions. Re-check appetite when the map gets bigger than the user's stated
   appetite.

4. **Record boundaries as no-gos, not silence.** Attach a source reference where one
   exists.

   ```sh
   provenance boundaries create --scope <scope> \
     --id boundary_<stable_slug> \
     --requirement-id <anchor_requirement_id> \
     --statement "<constraint on solution space>" \
     --source-id <source_id> \
     --source-clause "<clause or section>" \
     --format json
   ```

5. **Store fog as deliberately unstructured text.** Fog is the dim view of coming
   decisions that cannot yet be stated precisely.

   ```sh
   provenance requirements fog set --scope <scope> \
     --requirement-id <anchor_requirement_id> \
     --text "<not-yet-sharp investigations and worries>" \
     --format json
   ```

6. **Create topics and first questions.** Questions must be sized to one agent session and
   minted with the verb that resolves them: `grill`, `prototype`, `research`, `verify`, or
   `task`.

   ```sh
   provenance topics create --scope <scope> \
     --id topic_<stable_slug> \
     --requirement-id <anchor_requirement_id> \
     --title "<area of investigation>" \
     --format json

   provenance questions create --scope <scope> \
     --id question_<stable_slug> \
     --topic-id <topic_id> \
     --question "<precise question>" \
     --method grill \
     --format json
   ```

7. **Persist the charting thread.** Record the appetite, boundaries, fog, and question
   rationale on the requirement thread.

   ```sh
   provenance thread post --scope <scope> \
     --parent-type requirement --parent-id <anchor_requirement_id> \
     --role assistant \
     "CHART: appetite=<...>; boundaries=<...>; fog=<...>; first frontier=<...>" \
     --format json
   ```

8. **Handoff.** End with copy-paste next commands. Include parallel lines only for
   independent topics/questions.

## Mode 2: Work

Use Work when an anchor requirement already exists and the session should advance the
frontier.

### 1. Prime

Load the map low-res with the commands from **Start every session**. `provenance prime`
supplies rules and computed gaps (plus active threads only with `--include-threads`); load
the anchor graph, fog, boundaries, topics, and questions with their separate commands. The
shaping-focused subset of the computed graph frontier includes:

- requirements with neither a valid source reference nor a valid `references` edge;
- unresolved `contradicts` pairs;
- resolved requirements (including those with a resolving resolution) with no downstream
  Rule;
- open or `blocked_on_human` questions and open topics.

A no-downstream-Rule gap is a question about the obligation, not an instruction to write
a record. Ask what atomic behavioural obligation refines the Requirement. Land a Rule
only when that obligation is precise; whether production code implements it is a
separate question. An unimplemented Rule is an ordinary state and should remain visible
as such rather than being replaced by a vague Rule merely to close the gap.

Proposals are not part of the computed graph frontier and are not a batch-review inbox.
Claiming a topic atomically consults and returns undisposed `proposed` and `asserted`
proposals targeting the topic, its anchor requirement, the requirement's direct domain,
the topic's questions, or its explicit artifact links; each result includes its derived state
and deterministic reasons. If the topic claim is part of diff-driven work, repeat
`--changed-path <repo-relative-path>` on `topics claim`. Otherwise run `provenance proposals
surface --scope <scope> --changed-path <repo-relative-path> --format json` (repeat the path
flag).
Review only those surfaced proposals. Use a complete list only for a deliberately bounded
set of competing, contested, or conflicting proposals that jointly blocks the turn.

Do not hand-wire a private frontier in chat. If the graph says a different thing than your
notes, fix the graph or trust the graph.

### 2. Claim

Claim before work so concurrent sessions skip what you are touching.

| Method | Claim | Command |
|---|---|---|
| `grill` | whole topic | `provenance topics claim --scope <scope> --id <topic_id> --actor <agent> [--changed-path <path>...] --format json` |
| `prototype` | single question | `provenance questions claim --scope <scope> --id <question_id> --actor <agent> --format json` |
| `research` | single question | `provenance questions claim --scope <scope> --id <question_id> --actor <agent> --format json` |
| `verify` | single question | `provenance questions claim --scope <scope> --id <question_id> --actor <agent> --format json` |
| `task` | single question | claim its open question if represented; there is no separate task record or task claim |

If claim fails, do not work that item. Pick another frontier item or hand off that it is
already held.

On a successful topic claim, inspect `surfaced_proposals` before resolving the first
question. They are context that has become decision-relevant because the turn entered its
explicit territory; do not infer further territory from titles or graph proximity. If
lifecycle consultation fails, the topic claim is not persisted.

### 3. Resolve and land as you go

For each question, use the method on the question.

#### `grill`

Ask one precise question at a time and offer a recommended answer. When the human accepts,
land immediately before asking the next question.

Do not land a rich answer as a bare answered-question. Fan out into the graph:

```sh
provenance thread post --scope <scope> \
  --parent-type requirement --parent-id <anchor_requirement_id> \
  --role user \
  "Q <question_id>: <human answer>" \
  --format json

provenance resolutions create --scope <scope> \
  --id res_<stable_slug> \
  --title "<decision title>" \
  --requirement-id <anchor_requirement_id> \
  --position "<accepted position>" \
  --rationale "<why, including rejected alternatives>" \
  --status proposed \
  --made-by "<human or actor>" \
  --format json

provenance requirements create --scope <scope> \
  --id req_<spawned_slug> \
  --statement "<spawned requirement>" \
  --status discovery \
  --format json

provenance edges create --scope <scope> \
  --type spawns \
  --from-type resolution --from-id res_<stable_slug> \
  --to-type requirement --to-id req_<spawned_slug> \
  --format json

provenance questions answer --scope <scope> \
  --id <question_id> \
  --answer "<short answer gist; canonical decision is res_<stable_slug>>" \
  --resolution-id res_<stable_slug> \
  --format json
```

Only create the artifacts the answer actually implies. If it only answers the question,
`questions answer` is enough. If it implies a requirement, boundary, contradiction, or
source gap, land that too before continuing. If it implies a Rule, land the Rule and any
known bindings—next.

##### Landing a Rule and its bindings

A Rule is an identified atomic behavioural obligation refining a Requirement. A
Resolution may produce it. The Rule may exist before production code implements it or
evidence verifies it; those absences are Unimplemented and Unverified, not reasons to
change the artifact's kind.

1. **The record.** The statement names the precise behavioural obligation in one clause.
   `--source-document` and `--source-section` may cite the source behind the Rule; they
   never count as an implementation binding. Bind known implementation through a
   scanner-recognized attribute, helper, decorator, or comment.

   ```sh
   provenance rules create --scope <scope> \
     --id rule_<stable_slug> \
     --requirement-id <anchor_requirement_id> \
     --resolution-id res_<stable_slug> \
     --statement "<atomic behavioural obligation, in one clause>" \
     --severity high \
     --format json
   ```

2. **The bindings, when they exist.** `#[rule("<id>")]` sits on the primary production
   implementation; at most one function or type may carry an id for now.
   `#[verifies("<id>", <method>)]` sits on the evidence—`exhaustion`, `property`,
   `examples`, `conformance`, `construction`, or `proof`. `construction` goes on the type
   whose construction is the evidence, not on a test. Verification binds to the known
   Rule and does not require an implementation binding.

   ```rust
   #[rule("rule_<stable_slug>")]
   pub fn <bare_symbol>(...) -> ... { ... }

   #[test]
   #[verifies("rule_<stable_slug>", exhaustion)]
   fn <bare_symbol>_holds_over_every_input() { ... }
   ```

3. **The check.** The scan reports a binding that cites an unknown Rule, a second primary
   implementation claiming one id, and active Rules with no implementation or no
   verification anywhere.

   ```sh
   provenance coverage scan --path . --scope <scope> --validate-rules --format json
   ```

Unimplemented and Unverified are **absence**, derived by that scan and never written into
the record. How loudly a repo treats absence is its own dial: `--strict` makes any warning
fail the command, and a repo that runs the scan without it is reporting, not gating.
Neither setting is a reason to write a test that asserts nothing.

#### `prototype`

Use the `provenance-fork-tournament` skill. This is two-phase work:

1. Phase 1 spawns stance-based agents and lands proposals/contributions/synthesis.
2. Mark the question `blocked-on-human` and stop the session.
3. Phase 2 presents proposals, extracts reactions, lands the resolution, disposes of
   proposals with dispositions, spawns the requirements and Rules the direction actually
   implies, then continues or hands off. A tournament often settles direction above the
   atomic behavioural level, so do not assume it yields a Rule.

Do not present the artifacts in the same session that created them.

#### `research`

Read sources, docs, or code. Land evidence-backed claims through contributions/proposals
when the output is candidate material; create source references or resolutions when the
human-ratified answer is durable. If research incidentally answers a different open
question, answer that actual question too rather than burying the fact in a summary.

#### `verify`

Use adversarial refuters for claims that are expensive if wrong. Land refuter output as
contributions and synthesis: consensus, contested claims, minority objections, evidence
gaps, and required human decisions stay separate. If the result needs human disposal, mark
the question `blocked-on-human` and stop.

#### `task`

For human-world work, record the facts the task produced. Do not pretend the task is done
because instructions were given; land only completed facts, or leave the question open with
the blocker in its thread.

### 4. Graduate fog

After each landed answer, revisit fog on the anchor requirement.

- Keep fog if it still cannot be stated precisely.
- Create a Question when it can be stated precisely, even if blocked.
- Delete fog the answer killed.
- Do not pre-slice fog into speculative question nodes.

```sh
provenance requirements fog set --scope <scope> \
  --requirement-id <anchor_requirement_id> \
  --text "<remaining fog only>" \
  --format json

# If no fog remains:
provenance requirements fog clear --scope <scope> \
  --requirement-id <anchor_requirement_id> \
  --format json

provenance questions create --scope <scope> \
  --id question_<graduated_slug> \
  --topic-id <topic_id> \
  --question "<newly precise question>" \
  --method <grill|prototype|research|verify|task> \
  --format json
```

### 5. Stop and hand off

Stop at the first condition:

- context budget nearing threshold;
- human done;
- prototype or verify spawned a two-phase boundary;
- topic drained;
- claim conflict or missing source prevents clean work.

Before final response:

1. Clear claims you finished. These commands are alternatives, not a sequence: close a
   finished topic **or** release it if it remains open; answered questions clear their
   claims automatically, while unanswered questions should be released.

   ```sh
   provenance topics close --scope <scope> --id <topic_id> --format json
   provenance topics release --scope <scope> --id <topic_id> --format json
   provenance questions release --scope <scope> --id <question_id> --format json
   ```

2. Post a handoff on the requirement thread:

   ```sh
   provenance thread post --scope <scope> \
     --parent-type requirement --parent-id <anchor_requirement_id> \
     --role assistant \
     "HANDOFF: landed=<named artifacts>; remaining fog=<...>; frontier=<copy-paste commands>" \
     --format json
   ```

3. Tell the user what landed, what remains, and the next commands. Use artifact names and
   ids together, never bare ids.

## Landing checklist

Before moving to the next question, verify the current answer has been handled:

- thread message records the relevant dialogue;
- resolution records the accepted decision, if there is one;
- each precise atomic behavioural obligation is landed as a Rule; any known primary
  implementation and evidence carry `#[rule]` and `#[verifies]` bindings, and
  `coverage scan --validate-rules` reports any remaining absence honestly;
- spawned requirements are created and linked with `spawns` or `refines_into` edges;
- boundaries/out-of-scope rejections are explicit;
- source references and evidence are attached where they exist;
- proposal definitions remain immutable `proposed` records; assertion lineage uses assertion
  IDs, and only positive owned evidence with unblocked adjudication can create an assertion;
- accepted, rejected, or deferred outcomes are disposition records by a manifest-allowlisted
  actor ID. The ID is a repository-local audit attestation, not cryptographic identity. A
  canonical artifact must already exist under the exact scope and kind; correlate any external
  action with the generic system/scope/kind/key fields rather than workflow-specific metadata;
- fog is updated;
- invalidated questions are answered or updated; a thread note can document the change but
  does not change question status;
- claims are cleared on answered questions and closed topics.

If you cannot check these, do not continue the interview. Fix the graph first.
