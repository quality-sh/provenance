---
date: 2026-08-27T22:15:00+10:00
session: coordinator-authoring-pivot
git_commit: 2e88c9b8173cb8ed4400fc2be5c192da002463cf
branch: main
repository: quality-sh/provenance
topic: "Session Handoff: authoring-pivot epic coordination (provenance-46p / sfzf)"
tags: [handoff, session-transfer, qrspi, provenance-46p, provenance-sfzf]
status: complete
last_updated: 2026-08-27
last_updated_by: OpenCode coordinator agent
type: session_handoff
handoff_id: 2026-08-27_authoring-pivot-coordination
mint_server: https://mint.angelfish-celsius.ts.net
---

# Handoff: authoring-pivot epic coordination

This handoff transfers the COORDINATOR role for the Provenance authoring-pivot program.
The successor agent manages dispatched QRISPI workers, tracks bead states, runs consensus
loops with Ben, and cuts Plan/design briefs. Implementation work is performed by fresh
single-phase agents, never by the coordinator itself.

## 1. Where things live

- Local checkout: `/home/ben/Documents/repos/provenance` (branch `main`, was clean except noted files).
- Remote: `https://github.com/quality-sh/provenance`.
- Worker host: `mint` (Tailscale `https://mint.angelfish-celsius.ts.net`, basic auth).
  Credentials: source `~/.config/opencode/server.env` (`OPENCODE_SERVER_PASSWORD`).
- Launcher (fresh isolated branches): `mint-job --detach --model zai-coding-plan/glm-5.3-flash --variant high quality-sh/provenance "<brief>"`.
- Server API session control (bind to an EXISTING worktree):
  - Create session: `POST /session` with body `{"title": ...}` AND header
    `x-opencode-directory: <worktree-path>`. The body field `directory` is IGNORED;
    only the header binds location.
  - Post prompt: `POST /session/{id}/message` with the same header, JSON body
    `{"parts":[{"type":"text","text":...}],"agent":"remote-worker","model":{"providerID":"zai-coding-plan","modelID":"glm-5.3-flash","variant":"high"}}`.
    This call streams until generation ends: curl returns HTTP 000 / exit 28 at a
    ~20 s max-time. That is NORMAL and means delivery succeeded. Verify by growth of
    assistant messages or tokens, never by POST return code.
- Do NOT default models away: bare `mint-run` launches the Codex-default model. Always
  pin `zai-coding-plan/glm-5.3-flash` + variant `high` unless Ben says otherwise
  (he explicitly banned Opus usage this day).
- `claude-fable-5` produced empty-output sessions twice on mint today; treat as dead
  quota/auth on that account until retested.

## 2. Process laws (locked by Ben today)

1. **One agent per QRSPI phase.** Sessions die when their phase ends. Never send new
   instructions to old sessions. Today's five research flows ran Q+R+S in one session
   each as a grandfathered exception and are retired forever.
2. **Human review gates Plan/Implementation.** After Structure lands, nothing proceeds
   until Ben approves in conversation. Disposal outcomes become dated comments on beads;
   those comments are the normative spec handed to future phase agents.
3. **Citation spot-checking is standing practice.** One fabricated citation shipped in
   the `7ct` structure doc (a ghost quote attributed to `docs/cli.md:285-287`). Verify
   load-bearing claims against main before endorsing an artifact to Ben. State plainly
   when research tilts (the `a7d` artifact was judged slop by Ben and excluded entirely).
4. **ASD-STE100 plain English** is mandatory for all pushed artifacts. First drafts that
   failed readability were rewritten same-session on request.
5. **Beads hygiene**: file discovered-work as beads immediately; close decision-beads
   once packages are approved, referencing implementation successors; keep the Epic
   boundary strict (`46p` = authoring surface; `sfzf` = API surface delivery).

## 3. Architecture decisions locked today

Chronology matters - later entries override earlier ones. Recorded in comments on the
named beads; quote rather than paraphrase when briefing workers.

1. Architecture lock (epic comment, `provenance-46p`): SQLite = agent discovery surface
   reached through MCP server (direct SQL closed except one read-only escape-hatch
   command); JSONL = git-tracked truth, honestly sized for smaller projects, never the
   interactive read surface; SDK = one authoring mode producing migration-artifact
   scripts PLAYED ON MERGE; API verbs and direct JSONL remain legal alternative modes.
   Verdict recorded: truth is data, not code - IaC does not fit this product.
2. RBAC (bead `cvs`): basic coarse caps `{read, edit, execute}` plus manifest-write,
   manifest-resident grants `{actor_id, identity_type?, capabilities[], scopes[]}`,
   flat positive-only, deny_unknown_fields, humans-ratify preserved via identity_type
   validation, legacy `disposition_actor_ids` translated during exactly one
   protocol-bump window then refused ambiguous, grant edits land only via Git review,
   standard RBAC vocabulary ONLY (no invented terms; "Disposition Actor" slop banned -
   see closed bead `yzm`). Port intent recorded: primitives must map mechanically onto
   a future external OAuth-style authorizer for cloud-hosted Provenance.
3. Revision primitive (closed `7ct`): per-scope canonical-graph-digest CAS adopted,
   SHA stays history anchor, serials rejected (merge-driver union argument), spurious
   replan accepted as safe-by-default v1 cost, engine stays commit-free. Deferred:
   nothing ships until the transaction-kernel prototype needs it.
4. Query authority REVERSAL (reopened `1wh`, final decision wins over its own earlier
   approval): the coordinator initially recommended candidate D-contract-only (canonical
   shards stay served). BEN OVERRULED: SQLite becomes THE served read path because agent
   queries get complex and shard whole-corpus loads do not scale. Freshness leans
   auto-materialize-then-serve, annotate stamps, typed refusal only when catch-up fails.
   Incremental materialization is now a required first-class workstream (total DELETE +
   reload unacceptable steady-state). Second digest domain required covering all
   families SQLite stores (7ct scope digest excludes ideation). Eight sdk operations
   survive as contract presets re-backed by SQLite. The prior approval text on `1wh`
   items 1-2/6 is SUPERSEDED - the correction comment at 11:47Z is authoritative.
5. Envelope consolidation (closed `789`): Candidate B macro-stamping via
   `provenance_macros` + C trait fold-in; Route 1 fixture-equivalence schema sync only;
   keep `AssertionId`; leave graph_reference u32s untouched (standalone follow-up);
   pure-internals ratified (no protocol bump, no migration); zero TypeScript edits.
6. Threads/Messages PARKED (open `0i8`, execution pending): three-layer enforced marker
   (ADR naming reopening criteria tied to the near-future review-conversation horizon;
   `#[deprecated]` attributes; runtime refusal arm where `thread post` hard-fails).
   July-shard writer bug sealed deliberately - do NOT fix while parked. Wiki FieldNotes
   keep rendering. Full preservation ledger approved inside the parked artifact doc.
7. Near-future horizon (NOT current work, deliberately unbuilt; `7ct` closure encodes
   it): revisions route through proposals/change-kernel carrying executor-agent identity,
   sponsoring principal, task context, reviewer-list approvals, machine-generated
   permission-denial receipts as queryable Engine-derived durable records; domains double
   as authorization boundaries.
8. CLOUDFLARE roadmap talk happened but must NOT be tracked here - separate project.
   Do not create beads for it.

Epic tree after re-parenting (task-to-task blocking only in bd; epic edges are advisory):

```
provenance-46p  AUTHORING SURFACE [epic]
├── provenance-cvs    basic RBAC grants            [feature, P2, ready]
├── provenance-a8k    merge-play runner contract    [P2]
├── provenance-0i8    park threads/messages         [chore, P3, open]
└── closed: a7d(yzm-fold) · 7ct(deferred) · 789 · 633+hp7(re-filed)

provenance-sfzf  API SURFACE DELIVERY [epic, blocked-advisory-after 46p]
├── provenance-80qn   Operation Contract Layer      [epic, P1]
│     one definition generating TS client + MCP tools + CLI parity tests;
│     committed artifacts + CI drift gate (package-engine.js precedent);
│     closed vocabulary - non-goal 424 applies to generated surfaces;
│     consumes 789 traits, reserves 7ct wire names base_revision/expected_digest
├── provenance-1wh    SQLite flip + Relation vocab + defect fixes [task, P1]
│     MOVED HERE from 46p by Ben - deciding done, remaining work is API delivery
├── provenance-q82f   MCP server [P2] - phased behind 80qn (recorded in description)
└── provenance-mfd8   SQL escape hatch [P3] - loosest coupling
```

## 4. In-flight right now (Plan-phase workers, glm @high)

| Flow | Session id | Worktree directory | Base tip before plan push | Expected deliverable |
|---|---|---|---|---|
| `1wh` plan | `ses_fbcebc9b2ffe8mlPh0bykAQWu2` | `.local/share/opencode/worktrees/provenance/20260827T074514Z-80825ff0` | `0d7a81f` on branch `opencode/provenance-20260827T074514Z-80825ff0` | `docs/research/2026-08-27-qrspi-1wh-query-uniformity-plan.md` |
| `789` plan | `ses_fbce69342ffeSSPzbMiyFxwNtA` | `…worktrees/provenance/20260827T074100Z-b8dfd5ec` | `5b72cb9` | `…qrspi-789-record-envelope-plan.md` |
| `0i8` park runbook | `ses_fbce6433dffeea6LZV6Apg54L8` | `…worktrees/provenance/20260827T074046Z-b7797045` | `1950a2e` | `…qrspi-0i8-park-execution-plan.md` |
| `cvs` plan | `ses_fbce5ea1cffe8N5ZesrS9D6Q0O` | fresh detached branch, job `20260827T120257Z-63a48e55` | discover via `git ls-remote origin 'refs/heads/*063a48e55*'` | `…qrspi-cvs-basic-rbac-plan.md` |

Last observed token counts (22:09 local): 9.2k/2.9k/2.9k/2.3k output respectively, all
actively streaming. Collection recipe per flow:

```bash
# 1. finished?
curl -s -u "opencode:$PW" "$SERVER/session/$SID/message" \
  | jq '.[-1].info.finish'          # expect "stop"
# 2. confirm last assistant text carries the deliverable pointer
#    (they reply with branch + path + hash only)
# 3. confirm remote tip moved
git ls-remote origin 'refs/heads/opencode/*'
```

Pushes happen on their CURRENT branch; nothing merges without review. Review cadence is
Ben's choice: sequential as they land, or one batch sitting once all four stop.

## 5. Verification snippets (copy-paste live)

```bash
source ~/.config/opencode/server.env; SERVER=https://mint.angelfish-celsius.ts.net
# token pulse for any session id
curl -s -u "opencode:$PW" "$SERVER/session/$SID" | python3 -c "
import json,sys;d=json.load(sys.stdin);t=d['tokens']
print(d['model'], t)"
# last-message inspection (role / finish / tail)
curl -s -u "opencode:$PW" "$SERVER/session/$SID/message" | python3 - <<'PY'
import json,sys
msgs=json.load(sys.stdin)
info=msgs[-1]['info']; text=''.join(p.get('text','') for p in msgs[-1]['parts'] if p.get('type')=='text')
print(info.get('role'), info.get('finish'), '|', text[-120:])
PY
```

Session-model quirk: top-level `session.model.modelID` can render `None`; trust the
per-message `info.model` (correctly shows provider/model/variant actually used).

## 6. Pushed artifacts index (all reviewed or awaiting review)

Research (structure-complete) artifacts on branches as named in section 4 predecessors:

- `a7d`: `2026-08-27-qrspi-a7d-actor-vocabulary.md` — EXCLUDED from design input (Ben
  judged slop). RBAC direction replaced it via directive; do not resurrect.
- `7ct`: `2026-08-27-qrspi-7ct-graph-revision-primitive.md` — deslopped rewrite approved
  basis; contains ONE struck fabricated citation (`cli.md:285-287`) noted in disposal.
- `1wh`: `2026-08-27-qrspi-1wh-query-uniformity-read-path.md` — verified clean (six
  spot-checks passed). Its authority ruling superseded by reversal above; its DEFECT
  FINDINGS remain fully binding acceptance targets.
- `789`: `2026-08-27-qrspi-789-record-envelope-base.md` — cleanest artifact; corrected
  two stale coordinator claims; keep those corrections (no TS shrinkage possible;
  generator lives under `packages/provenance/scripts/`).
- `0i8`: `2026-08-27-qrspi-0i8-threads-messages-retirement.md` — includes approved
  PRESERVATION LEDGER (verbatim rule-text quoting etc.) and shelved REMOVE runbook.
- Coordinator companion: `docs/research/2026-08-27-sdk-authoring-surface-and-codemode-agents.md`
  (committed with this handoff) and Ben's agent ERD map
  `docs/research/2026-08-27-data-model-and-erd.md` (same commit - previously unpushed;
  workers on fresh branches could not see it, which triggered honest anchor-corrections).

## 7. Next actions queue for the successor agent

1. Poll section-4 flows; as each stops, verify branch tip advanced and doc exists.
2. Walk Ben through plans one-at-a-time (his stated preference); my suggestion stands:
   `1wh` first (substrate + flips everything else), then `789`, `cvs`, `0i8`. Consensus
   rounds work like today: he reads, you summarize/opine, dispose onto the bead as a
   dated normative comment, close if decided, cut implementation child when ready.
3. Implementation-phase dispatches, once their plans are approved, are fresh single-phase
   agents again - reuse this brief template's standing rules verbatim, swapping the
   deliverable name and stage line (`stage: plan-approved; executing` in front matter,
   deliverable being CODE commits instead of a doc, plus test evidence in reply).
4. When Ben green-lights the contract layer content: fire a FRESH Question/Research/
   Structure flow for `80qn` (it has had NO phases yet; nothing skipped). Seed inputs:
   `sfzf` scope seeds, architecture-lock triad, `789` outcome, non-goal 424 constraint,
   opencode CodeMode discovery-catalog reference links already in the sibling research
   doc. glm @high throughout.
5. Optional cleanup: empty probe session `ses_fbced400bffefEKDvWS9TQX70X` (title
   `qrspi-1wh-plan-probe`, zero messages) can be left to rot harmlessly.

## 8. Landmines (do not step on)

- Do not log ANYTHING Cloudflare/Durable-Object related - separate project, explicit.
- Do not resurrect requirements-as-code framing; IaC verdict is locked in the epic.
- Do not add fine-grained capabilities beyond the four coarse ones before the kernel
  horizon - `cvs` comment forbids it explicitly.
- Do not fix the July-shard month rolling bug - sealed by park decision.
- Do not reopen `a7d` research thread; its doc is radioactive, directives live on `cvs`.
- bd cannot remove-parent cleanly everywhere and refuses task->epic dep edges; work
  around with `--parent` flag (used successfully) and descriptive closures.
- Unrelated dirty working-tree files existed locally (`package.json`, `.claude/plans/`);
  leave them out of any commits you make from this checkout.

## 9. Session export status

OpenCode server-side sessions are authoritative on mint (chat transcripts, tokens);
this document intentionally duplicates only coordinator-relevant state. No
`~/.opencode/sessions/current/*` export machinery was used locally - the operational
source of truth is (a) this file, (b) the bead board (`bd` CLI anywhere the repo is
cloned; Dolt-backed `.beads/interactions.jsonl` ships with the repo), and (c) the mint
server API for live sessions. Nothing else requires restoration.
