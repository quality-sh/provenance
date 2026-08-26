---
name: provenance-fork-tournament
description: Run a fork tournament when a shaping session hits a genuine design fork — mutually exclusive directions, expensive to reverse, and the human's preference unknowable without concrete artifacts to react to. Implements the `prototype` resolution method from docs/shaping.md - spawn stance-based agents producing competing artifacts as proposals (phase 1, end session), then present them for human disposal and land the decision as a Resolution (phase 2).
---

# Fork tournament (`prototype`)

Implements the `prototype` resolution method. Canonical design: `docs/shaping.md`,
"Fork tournaments" and "The output contract" — where this file diverges, that one wins.

A prototype's job is to **raise the fidelity of the discussion**. The artifact is cheap,
concrete, and disposable — a conversation move, not a deliverable. The tournament serves
the dialogue; its primary output is the human's extracted reactions plus the recorded
decision. Never polish the artifacts; never treat the winner as shipped work.

## Is this fork genuine?

Run a tournament only when **all three** hold:

1. **Mutually exclusive** — the directions cannot be hedged or merged; picking one
   forecloses the others.
2. **Expensive to reverse** — later work will build on the choice; unwinding it means
   unwinding that work.
3. **Preference unknowable without artifacts** — if a grill question would settle it,
   grill instead. The tournament exists for the case where the human can only *react*,
   not *specify*.

A tournament fires at forks — **never a standing committee** on everything. If you are
reaching for it twice in one topic, you are probably under-grilling.

## Choosing N

2–4 stances. Default 2 for a binary fork. Add a third only when a genuinely distinct
value system exists — a direction someone could actually champion, not a strawman or a
midpoint. Never exceed 4: reactions blur, cost outruns fidelity gained, and the human's
disposal turn stops being cheap.

## Designing stances

A stance is a **value system + quality bar + exit criterion** — *not* a character or
celebrity. Each stance must know what it optimizes for, what "good" means under that
value system, and when to stop:

- "Reduce until it fails the 5-second test" (minimalist: strip until comprehension breaks, back off one step)
- "Disclose progressively by altitude" (layered: every detail reachable, nothing above its altitude)
- "Make the failure mode impossible, then earn back convenience" (safety-first)

Personas break sycophancy, license ruthlessness and taste, and force articulated values
before output. They do **not** provide independence — see Caveats.

**Engineer divergence deliberately**, because stances alone won't:

- **Evidence partition** — ground each stance in *different* material: different source
  documents, different prior resolutions, different reference designs. Assign the
  reading in the spawn prompt; forbid reading the siblings' material.
- **Task framing** — vary the verb, not just the values: one agent advocates a
  direction, another refutes the strongest one, rather than N parallel advocates.

## Phase 1 — turn N (agent work, no human)

Prerequisite: the fork exists as a Question with `resolution_method: prototype`.
Claim the **single question** before spawning (see the methods table in docs/shaping.md):

```sh
provenance questions claim --scope <scope> \
  --id <question_id> --actor <agent> --format json
```

1. **Spawn N agents in parallel** — one Agent tool call per stance, all in one message.
   Each spawn prompt carries: the anchor requirement and boundaries (loaded with the
   graph and boundary commands; `provenance prime` supplies rules and computed gaps),
   the question, the stance (values + quality bar + exit
   criterion), its evidence partition, and its task framing. Each agent produces one
   competing concrete artifact **opening with a design-principles manifesto** — the
   values stated before the output, so the human can react to the *why* as well as the
   *what*.

2. **Land each artifact as a contribution + proposal** linked to the question:

   ```sh
   provenance contributions create --scope <scope> \
     --id contrib_<question>_<slot> \
     --target-type question --target-id <question_id> \
     --participant-slot <stance_slug> \
     --stance support \
     --strongest-finding "<one-line: the artifact's central claim>" \
     --claims-json '[{"claim_id":"claim_<question>_<slot>","statement":"<central claim>","evidence_type":"artifact","evidence_reference_ids":["evidence_<question>_<slot>"]}]' \
     --evidence-json '[{"reference_id":"evidence_<question>_<slot>","evidence_type":"artifact","summary":"<what the artifact demonstrates>","file_path":"<artifact path>"}]' \
     --unsupported-recommendations-json '<speculation, explicitly marked>' \
     --uncertainty-level <low|medium|high> \
     --uncertainty-rationale "<why>"

   provenance proposals create --scope <scope> \
     --id prop_<question>_<slot> \
     --proposal-key <question>_<slot> \
     --proposal-type resolution_candidate \
     --title "<stance>: <artifact one-liner>" \
     --summary "<manifesto, then the artifact body or a file pointer>" \
     --target-type question --target-id <question_id> \
     --evidence-json '<same refs>' \
     --supporting-claim-id claim_<question>_<slot>
   ```

   Note `--stance` on contributions is the enum stance toward the target
   (`support|oppose|mixed|needs_more_evidence`), not the persona — a refuter-framed
   agent lands `oppose`. The persona's stance lives in `--participant-slot` and the
   manifesto. Claims cite typed evidence; speculation is explicitly
   `unsupported`/`exploratory` — per the output contract.

3. **Synthesize without averaging** — one packet: consensus, contested claims, and
   minority objections kept separate; the human decision explicit:

   ```sh
   provenance synthesis-packets create --scope <scope> \
     --id synth_<question> \
     --target-type question --target-id <question_id> \
     --summary "<the fork in one line; where stances converged and split>" \
     --consensus-json '<claims all stances landed on — convergence is signal, record it>' \
     --contested-claims-json '<the actual fork>' \
     --minority-objections-json '<kept, never averaged away>' \
     --suggested-artifacts-json '[{"proposal_id":"prop_<question>_<slot>","proposal_key":"<question>_<slot>","proposal_type":"resolution_candidate","summary":"<candidate summary>","origin_participant_slots":["<stance_slug>"]}]' \
     --required-human-decisions-json '[{"decision_key":"pick_<question>_winner","prompt":"Pick winner and grafts","blocks_promotion":true}]'
   ```

    Include one exact `suggested_artifacts` entry for every competing proposal. Each
    entry's `proposal_id`, key, and type must match its proposal definition.

4. **Mark the question blocked-on-human** and post the proposal ids to its thread:

    ```sh
    provenance questions update --scope <scope> \
      --id <question_id> \
      --status blocked-on-human

    provenance thread post --scope <scope> \
      --parent-type question --parent-id <question_id> \
      --role assistant \
      "BLOCKED-ON-HUMAN: fork tournament landed. Competing proposals: prop_<...>, prop_<...>. Awaiting disposal."
    ```

5. **END THE SESSION.** The fork spawn is a two-phase boundary — a stop condition in the
   Work loop (docs/shaping.md, "Work", step 5). Do not present the artifacts in the same
   session that produced them, and never proceed past the decision: the human hasn't
   ratified anything yet (invariant 3). Hand off.

## Phase 2 — turn N+1 (human disposal)

The promotion gate, with a clock. This is a grill-shaped turn against the artifacts.

1. **Present** — `provenance proposals list --scope <scope> --format json`; show each
   manifesto + artifact side by side. Lead with the contested claims from the synthesis
   packet, not a neutral tour.

2. **Extract reactions** — the reactions are the point. As the human reacts, post each
   onto the question's thread (`thread post ... --role user`) so nothing lives only in
   conversation state. Push past "I like B": *which property* of B, and what from the
   losers still matters?

3. **Land the decision the moment it resolves:**

   ```sh
   provenance resolutions create --scope <scope> \
     --id res_<question> \
     --title "<the fork, decided>" \
     --requirement-id <anchor_requirement_id> \
     --position "<winning direction + grafts>" \
     --rationale "<the extracted reactions: why winner won, why losers lost, what was grafted and from where>" \
     --input-type technical --input-reference prop_<winner> --input-summary "winning artifact" \
     --input-type technical --input-reference prop_<loser>  --input-summary "runner-up; grafted <idea>" \
     --made-by "<human>"
   ```

   One `--input-type/--input-reference/--input-summary` triple **per competing
   proposal** — this is what keeps grafted ideas traceable to their origin proposal. Run
   `--position` and `--rationale` through the `provenance-grounded-writing` skill's naming test before
   landing.

4. **Mark the question answered.** Creating the resolution does not update question state:

   ```sh
   provenance questions answer --scope <scope> \
     --id <question_id> \
     --answer "<winning direction + grafts>" \
     --resolution-id res_<question> \
     --format json
   ```

5. **Clear the winner's human gate and assert it.** After recording the human's decision,
   atomically replace the synthesis packet without its resolved blocking decisions and
   create the assertion. `--resolve-human-gate` preserves the rest of the packet's
   adjudication; first ensure the winner's supporting claim is not contested. Assert with
   the exact claim wired in phase 1:

    ```sh
    provenance proposals assert --scope <scope> \
      --id assertion_<question>_<winner_slot> \
      --proposal-id prop_<question>_<winner_slot> \
      --synthesis-packet-id synth_<question> \
      --supporting-claim-id claim_<question>_<winner_slot> \
      --resolve-human-gate \
      --decision-key pick_<question>_winner
    ```

6. **Dispose of every proposal** — winner accepted with the resolution as canonical
   artifact; losers rejected (rationale names the superseding resolution — see Gaps):

   ```sh
   provenance dispositions create --scope <scope> \
     --id pd_<question>_<slot> \
     --proposal-id prop_<question>_<slot> \
     --decision accepted \
     --rationale "<from the reactions>" \
     --actor-id <human_id> --actor-type human \
     --canonical-artifact-type resolution --canonical-artifact-id res_<question>
   # losers: --decision rejected --rationale "superseded by res_<question>; grafted: <idea>"
   ```

   This flips each proposal's `promotion_state` — no separate update step.

7. **Fan out** as any resolution does (docs/shaping.md, "Landing fan-out"): requirements
   spawned, fog graduated, rules only where a rule is real. Then continue the turn loop or
   hand off.

   A tournament settles **direction**, which is often broader than one atomic behavioural
   obligation. The winning artifact is a sketch the human reacted to, so the normal
   fan-out is spawned Requirements. Create a Rule only when the Resolution establishes a
   precise atomic obligation. The Rule may be Unimplemented; bind its primary production
   implementation with `#[rule("<id>")]` when one exists. **Do not mint a vague Rule only
   to make the decision look landed.** The Resolution is already the landing. See the
   `provenance-shaping` skill for Rule and binding guidance.

## Caveats (empirical — from the Statesman provenance scoping record)

- **Same-model personas converge** on central insights: the Steve and Jony canvas
  prototypes independently arrived at "territory, not pipeline." Do not expect
  independence from personas alone. Divergence comes from **evidence partition** and
  **task framing** — engineer both, every time.
- Convergence is still signal: when partitioned stances agree anyway, record it as
  consensus in the synthesis packet with its supporting participant slots and evidence
  references.
- What personas reliably provide: broken sycophancy, licensed ruthlessness and taste,
  values articulated before output. Use them for that; nothing more.
- **A tournament at forks — never a standing committee.** N artifacts cost N sessions
  of context; spend them only where a reaction is the only way to learn.

## CLI gaps and conventions (as of this writing)

- **Question status and method are first-class** — use `questions update --status
  blocked-on-human` for the phase boundary, and `questions update --method prototype`
  if an existing question was minted with the wrong method. Keep the thread post because
  proposal ids are not question link targets.
- **Proposal definitions are immutable and always `proposed`.** Before disposition, create an
  assertion only when the exact proposal suggestion has positive owned evidence and no
  contested claim or blocking adjudication. `dispositions` is the sole authority for
  `accepted|rejected|deferred`; reject losers with rationale naming the winning resolution.
  The actor ID must be repository-allowlisted and is an audit attestation, not a signature.
- **Generic `edges create` is available** and requires existing endpoints. Relevant valid
  directions are `spawns` (resolution → requirement), `produces` (requirement or resolution
  → rule), and `supersedes` (requirement → requirement). Proposals are not graph edge
  endpoints, so graft traceability to competing proposals rides on resolution input references.
