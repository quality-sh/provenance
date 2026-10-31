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

Before you call code redundant or delete tests, run
`provenance rules resolve-symbol --file <path>` on the file.

1. **Land every decision as it resolves.** Never carry a resolved-but-unrecorded decision
   in conversation state.
2. **Do not outrun context.** Stop before the map is too large to hand off accurately.
3. **Do not proceed past unratified decisions.** If the human has not accepted the
   position, it is a Proposal, open Question, or `blocked_on_human` fork, not a decision.
4. **Leave the map consistent at handoff.** Claims, fog, questions, and frontier must tell
   the next session what to do without reconstructing your chat.
5. **The tree reads as a spec.** At every handoff a developer can build from the anchor's
   requirement tree alone. See "The tree is the spec".
6. **Read the stored state at the start of every turn.** Before the first graph write of a
   turn, read the Topic, and each Question that you work on, with a `get` command in its
   own tool call. Do not filter that output: no `| head`, `| grep`, `| jq`, or other pipe.
   This applies to every turn that writes, also a turn that only claims, releases, or adds
   a thread note. See "Recover before each turn".

One question per session is **not** the invariant. Grill turns can drain a topic question
by question; expensive methods claim one question and often stop at a phase boundary.

## Check your CLI

Run this before the first graph write of a session:

```sh
provenance --version
```

An installed binary that is older than this skill accepts different flags, and it can
refuse work that the current CLI allows. One obsolete release required a Resolution
producer on every Rule, which pushed agents into inventing a Resolution for a Rule that a
Requirement alone had produced. A binary that does not understand `--version` at all is
far behind; reinstall it before you shape anything:

```sh
cargo install provenance-cli --force
```

Do not model the graph around what an old binary accepts. If a command and this skill
disagree, update the binary first, then shape.

## Start every session

1. Confirm the repo has a Provenance store. If absent, ask before initializing unless the
   user already asked you to set it up:

   ```sh
   provenance init --path . --scope <scope> --path-prefix .
   ```

2. **Open the requirements view first.** Do this before the first question and before
   the first graph write. The person must see each record land while they answer. Do
   not ask a question while the view is closed.

   The view shows the anchor inside the tree of its parent, not as a root. Open the page
   of the anchor's parent Requirement (the Requirement that the anchor refines). Use the
   anchor itself only when it has no parent. In Chart mode, create or select the anchor
   first (see "Chart workflow"), then open the view of its parent. Never open the page
   with the anchor as its root.

   Select the command from where the session runs:

   - **A desktop app with a browser pane** (Claude Code desktop, Codex desktop): start or
     reuse the host without the system browser, get the parent's link, and open
     `review_url` in the browser pane:

     ```sh
     provenance review --repo . --repository-id local --scope <scope> --detach --no-open
     provenance <parent_requirement_id> --scope <scope> --review-link --no-open --format json
     ```

   - **Claude Desktop or ChatGPT desktop** (MCP tools, no browser pane): open the MCP
     document view. Call the `get-requirement-document` tool with the parent's ID and show
     the result to the person. Call it again after each answer lands.

   - **A plain CLI session** (a terminal, `claude`, `codex`): start or reuse the host so
     that it opens the system browser, then show the parent's tree in that page:

     ```sh
     provenance review --repo . --repository-id local --scope <scope> --detach
     provenance <parent_requirement_id> --scope <scope> --review-link --format json
     ```

     Do not add `--no-open` in a CLI session. The Provenance section of `AGENTS.md`
     starts the host with `--no-open`; in a shaping session, this step replaces that
     command.

   Also give `review_url` in the chat as plain text. Tell the person to keep the page
   open and to watch the records land there after each answer. Keep the page open for
   the full session. The page follows records as they land, so do not open a new link
   for each record: a review link for a record below the anchor moves the page to a
   smaller tree. One host serves the repository and scope for all sessions, so leave it
   running. A link works once and expires after 24 hours; get a new link for the same
   parent when it has expired. The page is served on the machine that runs the host. If
   that is not the person's machine, say so, and give the person the plain CLI commands
   above to run on their own machine.

3. Read the domain guidance, then load the low-resolution map:

   ```sh
   provenance prime
   provenance rules list --scope <scope> --format json
   provenance gaps --scope <scope> --format json
   provenance graph <anchor_requirement_id> --scope <scope> --format json
   provenance requirements <anchor_requirement_id> get --scope <scope> --format json
   provenance topics list --scope <scope> --format json
   provenance questions list --scope <scope> --format json
   provenance boundaries list --scope <scope> --format json
   ```

4. Treat the map as an index, not a store. Keep only gists in your working context and
   zoom into the available graph and rule views with `provenance graph <requirement_id>`
   and `provenance traceability <rule_id>`; use the shaping list commands above for their
   full records. Refer to artifacts by meaningful names plus ids; never hand off bare ids.

## The tree is the spec

The anchor's requirement tree is the product spec. A developer must be able to build from
it without the chat. Each part of a record holds one kind of content:

- **Statement**: one obligation in one plain clause.
- **Description**: how the product behaves, in the present tense: its states, what moves it
  between them, what the person sees, and one example. Write it for a builder
  who has not read the thread.
- **Thread**: who said what. The person's words, the brief's words, and what changed go
  here, never in a description.
- **Fog and Questions**: what is not decided. A description does not list open items or say
  what remains to be shaped. Name an unknown in fog or mint a Question. A description may
  end with one `Open:` line that names them, so a builder knows where the spec stops.

A Question is answered only when the answer covers all of it. If an answer settles part of
a Question, land that part, leave the Question open, and narrow its text to what remains.

### The shape of the tree

The anchor is the top of the spec, not a list of Rules. Each decision adds one branch:

```text
parent Requirement
└── anchor Requirement
    ├── Resolution: the decision that settles a Question
    │   └── child Requirement (refines the anchor, spawned by the Resolution)
    │       ├── Rule (refines the child, produced by the Resolution)
    │       └── Rule (refines the child, produced by the Resolution)
    └── child Requirement for an accepted fact
        └── Rule (refines the child)
```

- A decision that settles a Question becomes a Resolution. The Resolution spawns a child
  Requirement: `--refines <requirement_id> --spawned-by <resolution_id>`. The child states
  what the product does because of the decision, not that a choice was made.
- The Rules of that decision go under the child Requirement, and each one names the
  Resolution with `--resolution-id`. The CLI accepts this link while the Resolution is
  proposed. Through it, the Resolution shows its downstream Rules in its impact view
  before anyone approves it. The impact view lists only these Rules, so every Resolution
  needs a spawned child Requirement and at least one Rule that names it. A Resolution
  without them is an orphan; do not leave one.
- An accepted fact that is not a choice becomes a child Requirement, and its Rules go
  under that child.
- Never put a Rule directly under the anchor. Never hang all the session's records off
  the anchor in one flat list.

### Every record goes to review

Submit every record that the session creates for review at once, so each one has its own
Approve control on the page. Requirement create opens its review submission by itself.
Rule, Resolution, Boundary and Source create do not. Submit each of these immediately
after you create it:

```sh
provenance <record_id> submit --scope <scope> --actor <actor> \
  --title "<short name of the record>" --summary "<what the record says>" \
  --builds-on-json '[]' --evidence-references-json '[]' --source-ids-json '[]' \
  --format json
```

Then check that `provenance <record_id> get --view review --scope <scope> --format json`
shows `review.decision.pending`. A guarded update of a record submits the new revision by
itself. Do not approve a record; approval stays with the person.

Before you hand off, read the tree as a builder would: read the anchor's requirement
document with `provenance requirements <anchor_requirement_id> document get --scope <scope>
--format json`, and read every statement and description in order. Fix a description that retells history, describes
another record's behaviour, or lists open items before you hand off.

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
     --description "<how the product behaves, as far as the brief says>" \
     --status discovery \
     --format json
   ```

   Create or select the anchor before the first question. Then open the requirements
   view of the anchor's parent (see "Start every session"). Create the anchor with
   `--refines <parent_requirement_id>` when the idea belongs under an existing
   Requirement.

2. **Attach known sources to the anchor.** Requirements with no valid source reference
   remain on the missing-source frontier, so add source references when evidence already
   exists.

   ```sh
   provenance requirements <anchor_requirement_id> update --scope <scope> \
     --relationships-json '{"cites":{"add":[{"source_id":"<source_id>","clause":"<clause or section>"}]}}' \
     --if-match <etag> \
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
     --source-ref-json '{"source_id":"<source_id>","clause":"<clause or section>"}' \
     --format json
   ```

5. **Store fog as deliberately unstructured text.** Fog is the dim view of coming
   decisions that cannot yet be stated precisely.

   ```sh
   provenance requirements <anchor_requirement_id> update --scope <scope> \
     --fog "<not-yet-sharp investigations and worries>" \
     --if-match <etag> \
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
   provenance requirements <anchor_requirement_id> discussions create --scope <scope> \
     --role assistant \
     --body "CHART: appetite=<...>; boundaries=<...>; fog=<...>; first frontier=<...>" \
     --format json
   ```

8. **Handoff.** End with copy-paste next commands. Include parallel lines only for
   independent topics/questions.

## Mode 2: Work

Use Work when an anchor requirement already exists and the session should advance the
frontier.

### 1. Prime

Load the map with the commands from **Start every session**. `provenance prime`
explains the domain. Use `provenance rules list` for Rules and `provenance gaps` for
computed gaps. Load the anchor graph, fog, boundaries, topics, and questions with
their separate commands. The
shaping-focused subset of the computed graph frontier includes:

- requirements with no valid source reference;
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
Claiming a Topic atomically returns undisposed `proposed` and `asserted` proposals in its
explicit territory. The CLI has no standalone proposal-surface command. For diff-driven
work, use `provenance proposals list --scope <scope> --format json` and inspect explicit
evidence paths. Do not infer territory from titles or graph proximity. Use a complete list
only for a deliberately bounded set of proposals that jointly blocks the turn.

Do not hand-wire a private frontier in chat. If the graph says a different thing than your
notes, fix the graph or trust the graph.

### 2. Claim

First complete the recovery reads below and inspect their output. Then claim before
work so concurrent sessions skip what you are touching. A claim is a graph write.

| Method | Claim | Command |
|---|---|---|
| `grill` | whole topic | `provenance topics <topic_id> claim --scope <scope> --actor <agent> --format json` |
| `prototype` | single question | `provenance questions <question_id> claim --scope <scope> --actor <agent> --format json` |
| `research` | single question | `provenance questions <question_id> claim --scope <scope> --actor <agent> --format json` |
| `verify` | single question | `provenance questions <question_id> claim --scope <scope> --actor <agent> --format json` |
| `task` | single question | claim its open question if represented; there is no separate task record or task claim |

If claim fails, do not work that item. Pick another frontier item or hand off that it is
already held.

On a successful topic claim, inspect `surfaced_proposals` before resolving the first
question. They are context that has become decision-relevant because the turn entered its
explicit territory; do not infer further territory from titles or graph proximity. If
lifecycle consultation fails, the topic claim is not persisted.

### Recover before each turn

At the start of EVERY shaping turn, read the stored Topic claim and the stored
method of each Question that you will work on in that turn. Do this before the
first graph write or Question work, including when the owner has already supplied
an accepted answer. Complete these reads as a separate tool step. Inspect the
output before any claim or write. Do not combine a recovery read and a claim or
write in one command or tool call. A get after a claim does not count, even when
both commands are in the same call. A read in the previous turn does not count. Repeat the reads
after a restart, a saved handoff, or context compaction.

Run these commands without output filters. Read the full JSON output. Do not pipe
recovery reads through grep, rg, jq, sed, head, or another filter. Do not extract
only the Question text, status, or answer: the claim and resolution_method fields
must remain visible. If output is cut short, repeat the record read with enough
output capacity before work. Read every selected Question, even if you will settle
several Questions from one owner reply:

```sh
provenance topics <topic_id> get --scope <scope> --format json
provenance questions <question_id> get --scope <scope> --format json
```

Check that the Question belongs to the selected Topic and is open. Check both claims
against your actor and the claim required by its method. Read `resolution_method` from
the Question. A Topic claim does not select a method for its Questions. A mixed-method
Topic can require a different method for the next Question.

If a required claim is absent, claim the item before work, then read it again. If another
actor holds a required claim, stop work on that item. If recovery is incomplete, stop
selection and recover the missing records. Read the bounded summary with
`provenance <topic_id> get --scope <scope> --view children --format json`. Use its
ready IDs only when `shaping.complete` is true. Filtered, depth-limited, truncated,
and unchecked reads are incomplete. Read all pages of the Topic and Question
collections when you need the map; a partial page cannot establish that work is absent.
Do not use conversation memory as a substitute for these reads.

A Question is ready only when every ID in `depends_on` names an answered Question
in the same Topic. An archived or human-blocked prerequisite does not satisfy it.
Closed and archived Topics have no ready Questions. Ready ID order is display
order, not priority. Claims restrict work, not discovery. Create or replace
`depends_on` through JSON input; an empty list clears it. Use prerequisites only
where order affects the answer.

Before you claim, answer, or land records for a Question, read each Question in its
`depends_on` list. If one of them is not answered, the Question is not ready: do not
claim it and do not land its decision. Tell the person which prerequisite is open, and
work on that prerequisite first.

Do not add a prerequisite to a Question that existed before your session, and do not
rewrite the text of such a Question. A new Question that you mint goes beside the
existing Questions, never before them. If a new Question must come first, ask the
person; do not change the stored order yourself. When the person answers an existing
Question, answer that Question in the graph, even if you also mint follow-up Questions.

Use the same fresh, unfiltered Topic read before a turn that only adds a thread note,
claims or releases the Topic, checks saved decisions, or writes a handoff. Such turns do not need a Question read unless
they work on a Question. A claim command or remembered handoff does not replace
the required get commands.

Apply the stored method to the selected Question. If the owner directly chooses
an answer to a prototype Question before prototypes are made, record that direct
choice and explain why it settles the Question without prototypes. Land every
accepted owner decision before asking the next question or ending the turn. For
each accepted decision in the owner reply, read the resulting graph records and
check that they state that decision. Check all decisions, including several
decisions in one reply. A thread note or a promise to write later is insufficient.
Use the record kinds that the answer requires; do not create duplicate records.
Do not add prerequisites that postpone an accepted decision. Preserve existing
prerequisites and claim restrictions. If they prevent landing, report the specific
blocker and keep the decision visible as unlanded; do not continue the interview.
Then read the stored state again. A proposed
answer that the human refuses remains unresolved. After a restart or compaction, recover
accepted decisions from the graph before you continue the dialogue.

### 3. Resolve and land as you go

For each question, use the method on the question.

#### `grill`

Ask one precise question at a time and offer a recommended answer. Record the
answer before the next question. Select records from its meaning, not from a fixed
sequence of commands.

1. Read the Question's Topic and its Requirement. Select the lowest Requirement
   whose obligation the answer concerns. Read its children before selecting it.
   If the answer concerns a different child, use that child for the discussion and
   new records. The session anchor is a map entry point, not the default parent.
2. Select only the records that the answer implies, in the shape that "The shape of
   the tree" shows:
   - An answer that supplies a fact with no product obligation needs only a Question
     answer.
   - A broad obligation needs a child Requirement under the selected Requirement.
   - A precise atomic behavioural obligation needs a Rule under a child Requirement,
     never directly under the anchor.
   - A decision that settles a Question, such as a choice among alternatives, needs a
     Resolution and the child Requirement that it spawns. State the unresolved problem
     in context, the choice in position, and the comparative reasons in rationale.
     Put source quotations in citations or inputs, not in place of those reasons.
3. Check each additional record for added meaning. The spawned child Requirement states
   the product behaviour that the decision sets, not "option X was selected". Do not
   create a second Requirement or Rule that only repeats the position. A precise
   obligation can correctly appear in a position and a Rule when the Rule adds an
   independently needed obligation.
4. Read review acceptance for the current content revision separately from lifecycle.
   A chat answer, a lifecycle value, and `made_by` do not prove that acceptance.
   Omit guessed authorship. Keep proposals proposed until the graph records review.
   An active Rule with a proposed Resolution can have independent grounding in a
   Requirement or Source; that pair of lifecycle values alone is not a defect.
5. Write descriptions and answer the Question as "The tree is the spec" says.

Record the dialogue on the selected Requirement:

```sh
provenance requirements <relevant_requirement_id> discussions create --scope <scope> \
  --role user \
  --body "Q <question_id>: <human answer>" \
  --format json
```

For a decision that settles a Question, create the Resolution, then the child
Requirement that it spawns, then the Rules under that child. Submit the Resolution and
each Rule for review at once (see "Every record goes to review"):

```sh
provenance resolutions create --scope <scope> \
  --id res_<stable_slug> \
  --title "<decision title>" \
  --requirement-id <relevant_requirement_id> \
  --context "<problem that remains without this choice>" \
  --position "<selected alternative>" \
  --rationale "<why this alternative meets the need better than the others>" \
  --status proposed \
  --format json

provenance requirements create --scope <scope> \
  --id req_<child_slug> \
  --statement "<what the product does because of the decision>" \
  --description "<states, what changes them, what the person sees, one example>" \
  --refines <relevant_requirement_id> \
  --spawned-by res_<stable_slug> \
  --status discovery \
  --format json

provenance rules create --scope <scope> \
  --id rule_<stable_slug> \
  --requirement-id req_<child_slug> \
  --resolution-id res_<stable_slug> \
  --statement "<atomic behavioural obligation, in one clause>" \
  --description "<trigger, result, what the person sees, one example>" \
  --severity high \
  --status review \
  --format json
```

`refines` identifies the parent; `spawned_by` names the producing Resolution and does
not replace `refines`. For a child Requirement that no decision produced, omit
`--spawned-by`. For a Rule that no Resolution produced, omit `--resolution-id`, and put
it under a child Requirement too. Then answer the Question with references to records
actually created:

```sh
provenance questions <question_id> answer --scope <scope> \
  --answer "<answer with references to the resulting records, if any>" \
  --format json
```

Examples:

- Direct Requirement: "The owner can set the mascot appearance." Add a child
  Requirement under appearance. No alternatives were compared, so no Resolution
  is needed.
- Direct Rule: "A share link expires after seven days." Add the atomic Rule under
  the link expiry child Requirement. Do not invent a Resolution producer.
- Decision with a spawned Requirement: select a robot instead of an animal because the
  robot can show a repair state without suggesting injury. Record the unresolved
  visual problem, the choice, and that reason in one Resolution under appearance. It
  spawns the child Requirement "The mascot is a robot that shows a repair state when
  the product fails." The Rule "The mascot shows the repair state while a sync error
  is open" goes under that child and names the Resolution.
- Decision with a separate obligation: select configurable expiry instead of a fixed
  duration because owners need different access periods. The Resolution spawns the
  child Requirement "The owner sets the expiry period of each link". Its Rules, for
  example "A link stops working at the end of its expiry period", go under that child
  and name the Resolution.

Land any Boundary, contradiction, or source gap that the answer also implies.

##### Landing a Rule and its bindings

A Rule is an identified atomic behavioural obligation refining a Requirement. A
Resolution may produce it. The Rule may exist before production code implements it or
evidence verifies it; those absences are Unimplemented and Unverified, not reasons to
change the artifact's kind.

1. **The record.** The statement names the precise behavioural obligation in one clause.
   `--source-document` and `--source-section` cite the source material the obligation was
   read out of, such as an award, a standard, or a policy clause. They are citations. They
   are not a planned home for the code, they reserve no file for it, and they never count
   as an implementation binding. Do not write an intended file path or symbol into them.
   Bind implementation, when it exists, through a scanner-recognized attribute, helper,
   decorator, or comment.

   A Requirement is enough to anchor a Rule. Pass `--resolution-id` only when a Resolution
   really produced the Rule. A Resolution producer is not required, and inventing one to
   satisfy the command misrecords how the obligation arose. If the CLI rejects a Rule that
   cites only a Requirement, the installed binary is out of date; see "Check your CLI".

   Rule creation defaults to `draft`. Keep this default for an agent-created Rule. Pass
   `--status active` only when a person ratifies the Rule.

   A Rule that a proposed Resolution produced names that Resolution with
   `--resolution-id` at once, and stays in review with `--status review`. Do not wait for
   approval to add the link: without it, the proposed Resolution shows nothing downstream.
   Only an approved Resolution can produce an active Rule or answer a Question. While the
   Resolution is proposed, answer the Question without a citation of the Resolution, and
   name the Resolution in the answer text.

   ```sh
   provenance rules create --scope <scope> \
     --id rule_<stable_slug> \
     --requirement-id <child_requirement_id> \
     --resolution-id <producing_resolution_id> \
     --statement "<atomic behavioural obligation, in one clause>" \
     --description "<trigger, result, what the person sees, one example>" \
     --severity high \
     --status review \
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
2. Mark the Question `blocked_on_human` and stop the session.
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
the Question `blocked_on_human` and stop.

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
provenance requirements <anchor_requirement_id> update --scope <scope> \
  --fog "<remaining fog only>" \
  --if-match <etag> \
  --format json

# If no fog remains:
provenance requirements <anchor_requirement_id> update --scope <scope> \
  --fog-json null \
  --if-match <etag> \
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

1. Read the tree as a builder would (see "The tree is the spec") and fix what does not
   read as a spec.
2. Clear claims you finished. These commands are alternatives, not a sequence: close a
   finished topic **or** release it if it remains open; answered questions clear their
   claims automatically, while unanswered questions should be released.

   ```sh
   provenance topics <topic_id> close --scope <scope> --format json
   provenance topics <topic_id> release --actor <actor> --scope <scope> --format json
   provenance questions <question_id> release --actor <actor> --scope <scope> --format json
   ```

   To archive a Question or Topic, use the commit that contains the
   change. Archiving a Topic also archives all Questions in that Topic.
   Discussions stay readable as history. Read the record first and pass its
   current ETag to the update. Merge a pull request that archives records
   with a merge commit. A squash or rebase merge drops the commit that the
   archive names.

   ```sh
   printf '%s' '{"status":"archived","archived_in_commit":{"commit":"<full_commit_sha>"}}' | \
     provenance questions <question_id> update --scope <scope> --if-match <current_etag> --stdin --format json
   printf '%s' '{"status":"archived","archived_in_commit":{"commit":"<full_commit_sha>"}}' | \
     provenance topics <topic_id> update --scope <scope> --if-match <current_etag> --stdin --format json
   ```

3. Post a handoff on the requirement thread:

   ```sh
   provenance requirements <anchor_requirement_id> discussions create --scope <scope> \
     --role assistant \
     --body "HANDOFF: landed=<named artifacts>; remaining fog=<...>; frontier=<copy-paste commands>" \
     --format json
   ```

4. Tell the user what landed, what remains, and the next commands. Tell them that the
   open page shows the tree, and give the link of the anchor's parent again (see
   "Start every session"). Use artifact names and ids together, never bare ids.

## Auditing a Rule

Preserve four independent facts when you audit:

- **Rule lifecycle** is `draft`, `review`, `active`, `deprecated`, or `archived`. It says
  where the Rule stands in review, not whether code or verification exists.
- **decision grounding** says why the obligation is legitimate. Source evidence, a
  Requirement, a ratified Resolution, or explicit human ratification can support a Rule.
- **implementation binding** links the Rule to primary production code when that code
  exists.
- **verification binding** links the Rule to evidence and its verification method when
  that evidence exists.

Do not derive one fact from another. In particular, code absence never tests whether the
Rule is valid. A finding that makes that inference calls a planned Rule invented.

**Decision fidelity: is the Rule true to what was decided?** Audit the Rule against its
sources, the Requirement it refines, and the ratified decisions that produced it.

- The statement gives one atomic behavioural obligation.
- Source evidence, a Requirement, a ratified Resolution, or recorded explicit human
  ratification supports that statement. `provenance traceability <rule_id> --scope <scope>
  --format json` prints the available upstream graph chain.
- No ratified decision disagrees with it.

A Rule that fails decision fidelity is unsupported. Report it, and name the source,
Requirement, or decision that it disagrees with.

**Implementation fidelity: does the code do what the Rule says?** Audit the code against
the Rule.

- `provenance coverage scan --path . --scope <scope> --validate-rules` reports an active
  Rule that has no primary implementation as unimplemented. Only that canonical scan
  derives the code-binding absence findings.
- Unimplemented is absence. It is an ordinary state of a planned obligation.

Prime and traceability are graph reads. They do not scan code, so they must not assert an
implementation or verification verdict. They can explain that an active Rule with no cited
code evidence is allowed to be unimplemented; the canonical scanner must run before you
report that derived state.

**Never report a Rule as invented, invalid, or unsupported because no code implements it.**
Code absence is evidence about implementation fidelity only. Provenance is planning-first:
Rules are written before the code exists, and each one is unimplemented until a person
writes that code. An audit that reads this absence as invention reports the method as a
defect.

When agent-authored behaviour has no source evidence, Requirement, ratified Resolution, or
explicit human ratification behind it, leave it where a human can still refuse it. Keep the
Rule `draft` or `review`, keep the change a `proposed` proposal, or keep an open Question
against it. Do not make it `active` to clear a report.

## Landing checklist

Before moving to the next question, verify the current answer has been handled:

- thread message records the relevant dialogue;
- each new Requirement and Rule has a description that states behaviour in the present
  tense, with no history and no list of open items;
- a Question marked answered is covered completely by its answer;
- the requirements view is open where the person can see it, with the anchor inside its
  parent's tree;
- resolution records the accepted decision, if there is one, and spawns a child
  Requirement; its Rules are under that child and name it with `--resolution-id`, so
  its impact view is not empty;
- no new Rule is directly under the anchor;
- every new Requirement, Rule, Resolution, Boundary and Source has a pending review
  submission;
- each precise atomic behavioural obligation is landed as a Rule; any known primary
  implementation and evidence carry `#[rule]` and `#[verifies]` bindings, and
  `coverage scan --validate-rules` reports any remaining absence honestly;
- child Requirements add meaning and use `refines` for their parent; `spawned_by`
  names only a producing Resolution;
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
