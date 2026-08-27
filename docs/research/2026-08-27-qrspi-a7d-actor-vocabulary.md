---
date: 2026-08-27
bead: provenance-a7d
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

provenance-a7d — Research: one shared actor and ownership vocabulary (epic provenance-46p)

=== QUESTION ===

**Sharp restatement.** What minimal, unified vocabulary for actors, principals, integrations, owners, and their relationships lets Provenance describe (1) reconciliation ownership (`declared_by`), (2) disposition attestation (allowlisted `DispositionActor`), and (3) the `{human, agent, service}` taxonomy — each retaining its exact present meaning — while stating precisely which object a future Planned Change `Approval` references and which claims that approval may carry?

**Sub-questions the answer must settle conceptually:**
1. Which terms are load-bearing (must appear in the glossary with fixed relationships), and which current strings (`declared_by`, `disposition_actor_ids`, `claimed_by`, `made_by`) map onto which term?
2. Are the spheres three roles of one underlying concept, or distinct concepts that merely share a string namespace?
3. What does an `Approval` reference — a Principal? a Planned Change digest? both? — and what may it claim without crossing the attestation posture (ADR 0001)?
4. Can machine (agent/service) principals carry approvals while the system honestly says "this is attestation, not human-proof"?
5. Does the external-action correlation tuple serve as the audit link for approvals performed outside this system?
6. Do low-ceremony working-state writes remain exempt from approval as a matter of policy rather than mechanism?

**Scope boundaries.**

In scope:
- Vocabulary definitions and relationship statements (glossary/ARCHITECTURE level, interface-and-invariant sketches only);
- Mapping every existing identity-bearing mechanism onto the vocabulary with zero behaviour change;
- Interface-level specification of the Approval carrier's referenced object and permitted claims;
- Explicit separation predicates (ownership-by-equality vs authority-by-configuration vs attestation-by-recording).

Out of scope:
- Code edits, schema/version changes, storage migration, TypeScript SDK redesign details;
- Choosing approval scopes/carriers (research-doc open decisions 7–8, provenance-46p siblings);
- Cryptographic signatures, authentication, or deriving human identity from the allowlist (explicit non-goal, research doc line 431);
- Resolving the Proposal/Change naming conflict (separate named conflict, research doc lines 98–111);
- Picking a winning candidate here — this stop is Structure only.

**Decisions enabled for the human reviewer:** which vocabulary candidate becomes the CONTEXT.md glossary update; whether `declared_by` remains permanently outside any authorization concept; whether `Approval` binds a principal and a digest as separate referenced objects; whether working-state exemption is codified as policy-table text; whether a Principal must exist in a registry or is a claim-shaped attestation.

=== RESEARCH ===

**Sphere 1 — Reconciliation ownership (`declared_by`).**
- `docs/state-format.md` L9–L18: `declared_by` "names the integration allowed to reconcile that record; it does not grant ownership of the scope or of unrelated graph state"; implicit takeover refused; `adopt_unowned` demands an explicit Stable ID plus pre-matching definition; adoption changes only `declared_by` and `declaration_address` — schema version 1 unmoved.
- `docs/adr/0008-declaration-adoption-is-explicit.md` L14–L17 (refusal is a safety control against one spec taking another process's records), L26–L32 (adoption applies only to unowned records; "It does not transfer a record from one owner to another"), L49–L52 (foreign-owned record ⇒ conflict, no write).
- `crates/provenance-store/src/state_store/typed_specs/adoption.rs` L311–L317: `rejects()` — same owner ⇒ refuse an inexact adoption request; **foreign owner ⇒ always refuse**; unowned ⇒ require requested+exact. Target validation L338–L420 (exact Stable ID, exactly one matching declaration, canonical record must exist).
- The owned objects: `crates/provenance-core/src/model/artifacts.rs` L224–L231 (Source), L284–L291 (Requirement), L404–L411 (Rule) — each carries `declared_by: Option<String>` + `declaration_address`; bindings too: `model/integrations.rs` L115 (`VerificationRun`), L139 (`VerificationBinding`), L177 (`ImplementationBinding`). One owner per document: `crates/provenance-core/src/protocol/typed_spec.rs` L20–L32 (`TypedSpecInput.declared_by`).
- Glossary anchors: `CONTEXT.md` L99–L101 (Declaration owner = "the integration URI… grants no authority over other records"), L103–L105 (Declaration address), L107–L109 (adoption never transfers between owners). Test fixture shape `"test://typescript"` (`model/tests/integrations.rs` L34) confirms URI-naming-an-integration usage.

**Sphere 2 — Disposition attestation (manifest allowlist).**
- `crates/provenance-core/src/model/manifest.rs` L25–L43: `Manifest.disposition_actor_ids: Vec<String>`, defaults empty.
- Enforcement: `crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs` L144–L158 (doc comment: only manifest-named actors may dispose a live proposal; empty list blocks everything — safe default; "**the actor id is an attestation, not proof of identity. Nothing here checks a key or a signature**"), L159–L174 (`validate_actor_allowlists`), L176–L195 (rule `rule_disposition_actor_allowlist`; fires only when effective pre-disposition state is `proposed`/`asserted`), L197–L207 (explicit empty-allowlist failure message naming `provenance init --disposition-actor-id`).
- Posture statements: `docs/adr/0001` L14–L17 (allowlist membership required; version 1 trusts repository/CLI access; "does not claim cryptographic signatures, account authentication, or proof that a human controlled the supplied ID"); `docs/cli.md` L396–L399 ("local audit attestation, not cryptographic authentication"; re-init preserves; `--clear-disposition-actors`); `CONTEXT.md` L135–L137 (Disposition).
- Wiring breadth (every write path validates the aggregate): CLI flags `crates/provenance-cli/src/cli.rs` L41–L47; init persistence `handlers/repo.rs` L55–L58 and L82–L86; check `handlers/check.rs` L100; import `handlers/import.rs` L66; store writers `state_store/proposal_writers.rs` L157/L184/L251; swarm batches `state_store/ideation_batches.rs` L110–L129; actor parsed into record at `handlers/dispositions.rs` L40–L44.

**Sphere 3 — Classification taxonomy (`IdentityType`).**
- `crates/provenance-core/src/model/ideation.rs` L74–L93: `IdentityType {Human, Agent, Service}`.
- Used **only** by `DispositionActor {identity_type, id, name}` (`model/ideation/dispositions.rs` L6–L13; record field L40) — grep across `crates/` finds no other consumer.
- Important nuance found: the taxonomy is **not** inert classification. `disposition_requires_prior_assertion` (`dispositions.rs` L60–L81) exempts an acceptance from the assertion requirement iff `identity_type == Human && canonical_artifact.is_some()` ("ratification through action", `CONTEXT.md` L139–L141; exercised in `state_store/tests/proposals/disposition_write_gate.rs` L134, L175–L201, and by the fork-tournament skill flow below). Moving or generalizing this enum is therefore behaviour-carrying, not cosmetic.

**Identity-bearing surfaces beyond the brief's three** (found while hunting contradictions):
- Shaping claims: `topics claim --actor <name>` described as "Actor name recorded on the claim" (`cli/shaping.rs` L60–L74; second site L160–L170); validated only as non-empty (`state_store/shaping_writers.rs` L171–L195, L231–L249, L456–L459) — no allowlist, no taxonomy, stored as `claimed_by`.
- `Resolution.made_by: Option<String>` display attribution (`model/artifacts.rs` L369–L376; listed among preserved optional fields in `docs/state-format.md` L7).
- Swarm participants act via `participant_slot` strings, and swarm output structurally cannot hold dispositions (`skills/provenance-swarm-backtrace/SKILL.md` L16–L21, L184, L280; ADR 0001 L25–L27 "swarm output cannot supply disposition authority").

**Machine identities acting today (workflow skills):**
- Fork tournament phase 2: the human disposal lands `dispositions create … --actor-id <human_id> --actor-type human --canonical-artifact-type resolution` (`skills/provenance-fork-tournament/SKILL.md` L204–L214), with L252–L256 restating allowlist-plus-attestation ("not a signature"). This is the sole path where `IdentityType::Human` currently unlocks a semantic shortcut.

**Planned-change direction (forward pressure):**
- `docs/research/2026-08-27-programmable-graph-change-proposals.md`: `Approval` = "An actor's acceptance of one Planned Change digest. Approval of a digest is not approval of the Change Program that produced it" (L82–L85); illustrative seam `plan(ChangeSet, BaseRevision, Principal)` / `commit(…, ExpectedDigest, Approvals)` (L119–L125, marked illustrative); invariant 3 "approval binds to normalized operations… not source code" (L156); PM scenario steps 4–6 (semantic summary → manager approves digest "through an allowed approval carrier" → trusted adapter submits "principal and approval") (L352–L357); failure cases include "approval is absent, revoked, malformed, or for another change" and "principal lacks authority for an operation" (L404–L420); state-class table separating Graph intent (approval-policy possible) from Working state ("usually no approval ceremony") with "Approval is policy over a transaction, not the transaction mechanism itself" (L251–L267); fog caveat "must not force a PM to approve every fog edit" (L288–L295); non-goal "Do not claim cryptographic human identity from the current actor allowlist" (L431); open decisions 7–8 (which scopes need approval; which carriers suit a manager without GitHub access) (L444–L445).

**Audit link for external approvals:**
- `ExternalActionCorrelation {system, scope, kind, key}` (`dispositions.rs` L22–L29); identity is the exact four-part tuple; "It is audit context, not Disposition identity or workflow state" (`CONTEXT.md` L95–L97; `docs/state-format.md` L92–L97; ADR 0001 L35–L38; `docs/cli.md` L420–L425: duplicate dispositions cannot mutate it).

**Contradicting evidence sought and result:**
- No code authorizes anything by `declared_by` beyond reconciliation (only adoption/reconcile paths read it) — supports keeping ownership out of authorization.
- `Principal` appears nowhere in `crates/` (rg: zero matches) — the term is purely prospective; no hidden collision.
- The allowlist gates nothing except dispositions (all call sites feed `validate_ideation_scope*` only) — no existing coupling an approval could silently inherit.
- Genuine tensions found: (a) the story "three spheres" undercounts — `claimed_by` and `made_by` are a fourth attestation-flavoured surface with no vocabulary at all; (b) `IdentityType` already drives a semantic gate (assertion exemption), so treating it as disposable classification is unsafe; (c) a shared noun **already exists informally**: code, CLI help, docs, and skills all say "actor" for four different things (claim holder, disposition attester, free-text made_by, integration owner is called something else again) — the collision is real today, not hypothetical.

**Evidence-split checklist.**

Repository facts (each cited above):
1. `declared_by` = per-record integration reconciliation right; foreign takeover always refuses; adoption is the sole ownerless transition (state-format.md L9–L18; ADR 0008; adoption.rs L311–L317).
2. `Manifest.disposition_actor_ids` gates dispositions of live proposals only; empty = block all; attestation not identity (manifest.rs L25–L43; aggregate_validation.rs L144–L207; ADR 0001 L14–L17).
3. `IdentityType{human,agent,service}` lives only in `DispositionActor` but materially controls the assertion-exemption gate (ideation.rs L74–L93; dispositions.rs L60–L81).
4. Free-form actor strings exist on topic/question claims (`claimed_by`) and resolutions (`made_by`) with no allowlist or typing (shaping_writers.rs L171–L249, L456–L459; artifacts.rs L369–L376).
5. Swarms cannot dispose; humans ratify through action with a canonical artifact (swarm-backtrace SKILL.md L184; fork-tournament SKILL.md L204–L214; CONTEXT.md L139–L141).
6. `ExternalActionCorrelation` is a closed four-string audit tuple, immutable after write (dispositions.rs L22–L29; CONTEXT.md L95–L97).
7. The research doc introduces `Principal` and `Approval` as candidates only; approval binds a digest, not source; cryptographic-human-identity derivation is an explicit non-goal (research doc L82–L85, L156, L431).
8. No `Principal` exists in code yet; the allowlist gates nothing but dispositions.

My inference (clearly not repository fact):
- The three spheres share zero predicate logic: ownership is string-*equality* scoped to one record; authority is *membership* in manifest configuration; attestation is *recording* a claim under trust-of-repo-access. Unifying them into one knob is a design choice, not a discovery.
- The natural join key between spheres today is nonexistent — they never meet, which is precisely why one glossary can be additive rather than migratory.
- Any future `Approval` can only ever reference a *claimed* identity (attestation posture), so "authority" in the approval sense degrades gracefully to configuration + audit, never to proof.
- The hazard in the brief ("collapsing reconciliation ownership into authorization") is concrete: `rejects()` equality-on-`declared_by` protects concurrent-integration safety, while allowlist membership protects decision authority; merging them would let whoever owns a record implicitly dispose/approve, contradicting ADR 0001's posture and giving integrations decision rights nobody granted them.

=== STRUCTURE ===

Four candidate resolutions follow. None is endorsed; all are designed so every existing mechanism retains meaning unchanged.

---

**Candidate A — Four-term split: Actor / Principal / Integration / Owner.**

*Position.* Distinct mechanisms get distinct nouns. An **Actor** is whoever-performed-an-action as recorded in an audit row (today's `DispositionActor`, `claimed_by`, `made_by` usages). A **Principal** is an identified party *referred to* by attestations and approvals (id + optional taxonomy). An **Integration** is the machine party named by `declared_by`, reconciling records it owns. **Owner** is not an entity but a role: the relationship "record R is owned by Integration I".

*Mechanism sketch (interfaces/invariants only).* Glossary + lightweight types: `PrincipalRef = {id: String, identity_type?: IdentityType}`; `Approval = {principal: PrincipalRef, digest: Digest}` (closed record); `declared_by` stays a plain string typed conceptually as Integration-URI. Invariants: (i) ownership checks remain equality-on-`declared_by` with ADR 0008 transition rules; (ii) authority checks remain manifest-membership; (iii) no check may derive one from the other; (iv) an Approval validates well-formedness + digest match only, never human-ness.

*Existing behaviour preserved.* All refusal messages, thresholds, and record shapes unchanged; the vocabulary is documentation-first, code-aliasing later and optional.

*Tradeoffs.* Highest fidelity to code reality; each term has a crisp referent. Cost: four terms to teach, and "Actor vs Principal" will invite questions.

*What makes it wrong.* If glossary growth outruns mechanism (no transaction seam materializes, so `Principal` dangles), or if "Integration ⊂ Principal" tempts a future unify-the-checks refactor — the collapse hazard again.

---

**Candidate B — Two-layer minimalism: Identity + Authority; retire "Actor".**

*Position.* Everything acting is an **Identity** (`{id, kind?}`). Every mechanism differs only in the **Authority** bound to that identity: `ReconcileAuthority` (per-record, by equality), `DecideAuthority` (manifest membership), `ApproveAuthority` (digest-bound claim). Cross-authority implication is forbidden by construction: holding one grant says nothing about another.

*Mechanism sketch.* One name/type for identity everywhere an actor string appears; three authority predicates named separately, each owning its own failure message. `Approval := Identity × Digest` plus optional carrier evidence. Working-state operations get revision-checked writes with `ApproveAuthority` absent — exemption expressed as "no grant required", not "mechanism skipped".

*Existing behaviour preserved.* Validation logic, messages, and persistence unchanged; docs rename prose ("declaration owner" → ReconcileAuthority phrasing) while field names stay.

*Tradeoffs.* Smallest noun count, sharpest statement of the non-implication invariant. Cost: it re-explains documented terms (CONTEXT.md "Declaration owner", "Disposition") and hides the equality-vs-membership difference inside one abstraction — the very difference that must not blur.

*What makes it wrong.* If implementers treat the three Authorities as interchangeable capabilities (one enum, one UI), the collapse happens quietly; also wrong if doc churn lands before the transaction module gives "ApproveAuthority" a real consumer.

---

**Candidate C — Freeze everything; add only Principal + Approval as forward references.**

*Position.* The spheres never merge syntactically. CONTEXT.md definitions of Declaration owner/address/adoption, Disposition, and IdentityType stand untouched. Exactly two new terms arrive, defined only for the planned-change layer: **Principal** = the party an attestation or approval names going forward (id, optional taxonomy, attested-not-proven); **Approval** = `{principal_ref, planned_change_digest}`, referencing nothing else and implying nothing else.

*Mechanism sketch.* `Approval` is a closed record whose engine-side checks are presence, well-formedness, and digest match; the manager-without-GitHub scenario is satisfied by any carrier able to produce a verifiable Approval reference. Approvals performed outside this system link back through the existing `ExternalActionCorrelation` tuple (system/scope/kind/key) carried as audit-only evidence — reused semantics, no new identity concept. Working-state ceremonies stay exemption-based via the state-class policy table ("approval is policy, not mechanism").

*Existing behaviour preserved.* Trivially — nothing renames, retypes, or re-gates.

*Tradeoffs.* Lowest risk, immediate applicability to provenance-46p siblings. Cost: `claimed_by`/`made_by` remain vocabulary orphans, and no rule is established for when a new mechanism may reuse an old term versus mint a new one — the ambiguity this bead exists to remove likely recurs at the next seam.

*What makes it wrong.* If the epic's intent was cleanup of today's informal "actor" collage (four unrelated fields sharing one word), deferring leaves the job half-done; also wrong if future approval code starts accepting `declared_by` URIs as principals absent a stated mapping rule — this candidate pins no such rule unless the human adds one.

---

**Candidate D — Registered principals with enumerated capabilities.**

*Position.* Add an optional manifest-level `principals` section: stable ids, taxonomy kinds, and explicit capabilities (`reconcile:<owner-uri>`, `dispose`, `approve:digest`). Ownership strings and the disposition allowlist become derivations from registrations rather than independent knobs.

*Mechanism sketch.* `init` grows principal registration; `declared_by` values resolve against registrations for reporting (validation stays permissive for legacy repos); `disposition_actor_ids` becomes a projection of `dispose` capability; `commit(…, Approvals)` checks the approving principal holds `approve:digest`. Invariants: registration adds information, never removes access (existing manifests behave identically without registrations); capability checks fail closed only where checks exist today.

*Existing behaviour preserved.* Manifest schema is extensible (serde-defaulted, cf. how `disposition_actor_ids` was added at manifest.rs L29–L30) and record shards stay at version 1.

*Tradeoffs.* Strongest long-term coherence: one place answers "who exists here", approvals gain a real authority notion ("principal lacks authority" becomes checkable, research doc L409), and cross-sphere reporting ("this integration and this approver relate to this change") becomes queryable.

*What makes it wrong.* It drifts toward account management — territory ADR 0001 deliberately avoided claiming ("does not claim… account authentication"); it adds a registration ceremony the PM-without-GitHub scenario doesn't require; unregistered repos grow a two-tier notion of who exists; and it pre-decides approval-scope questions the human explicitly kept open.

---

**Required analyses across candidates (where each stands):**
- *Ownership ≠ authorization:* A/C keep the predicates in separate names outright; B encodes non-implication as a stated invariant but risks soft merges; D needs its "registration adds nothing where checks exist today" invariant policed hardest.
- *Agent principals carrying approval without human-proof:* viable in all four because Approval validates only well-formedness + digest; `identity_type` travels as a self-declared attribute, and no gate may branch on it for approvals (only the existing ratification-through-action gate may branch on it, unchanged).
- *External-action tuple as approval audit link:* A, C, D attach it to approvals as optional audit evidence reusing the closed four-part tuple; B abstracts it inside ApproveAuthority evidence. None makes it identity or workflow state, per CONTEXT.md L95–L97.
- *Working-state ceremonies stay exemption-based:* all four express exemption as policy ("no ApproveAuthority/Digest required for working-state classes"), preserving the research doc's fog position (L266–L267, L288–L295).

**Decisions left explicitly to the human reviewer:**
1. Choose a candidate (or a hybrid, e.g., C now + B's non-implication invariant recorded as a rule).
2. Must a Principal exist in a registry (D) or is claim-shaped attestation sufficient (A/B/C)? This decides whether `init` grows any surface at all.
3. Does `declared_by` permanently stay outside the Principal namespace (recommended by A/C), or is a documented mapping "Integration-URI ⇒ service Principal" wanted for reporting?
4. Do the orphaned free-text surfaces (`claimed_by`, `made_by`, participant slots) get folded into the chosen vocabulary now or documented as deliberately informal?
5. May an Approval optionally carry an `ExternalActionCorrelation`-shaped evidence field, or does out-of-system approval linkage ride solely on dispositions for now?
6. Confirm `IdentityType` stays disposition-local despite its gate role (moving it is behaviour-carrying — see Research §3) or schedule a deliberate promotion with the transaction work.
7. When does CONTEXT.md change land: immediately as pure glossary text, or together with the first transaction-seam artifact so new terms ship with a consumer?
8. Approvals-performed-outside-the-system: accept the external-tuple audit link as sufficient, or defer entirely alongside open decisions 7–8 (carrier list, approval scopes)?

No worktree files were modified; this report is the sole artifact.
