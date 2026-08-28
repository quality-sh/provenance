# Domain Glossary

## Domain

A reader-facing taxonomy classification for requirements. A derived rule belongs to each Domain of its upstream requirements through the canonical graph relationships.

## Requirement

A higher-level obligation the system must satisfy. Requirements are refined into Rules.

## Resolution

A recorded decision that removes ambiguity. A Resolution may produce a Rule when the decision establishes a precise behavioural obligation.

## Rule

An identified atomic behavioural obligation that refines one or more Requirements and may also be produced by a Resolution. A Rule can exist before any Implementation binding or Verification.

## Implementation binding

A durable relationship from a Rule to production code that realizes it. A retired Implementation binding preserves a former claim as history but does not implement the Rule in active views. A Rule without an active Implementation binding is unimplemented, but remains a Rule.

## Verification

Evidence supporting belief that a Rule holds. A Rule can have no Verification; this absence is Unverified rather than a different kind of Rule.

## Verification binding

A durable relationship from an owner-local test key and repository code location to a canonical Rule. It records what is intended to verify the Rule, not whether an execution passed. A retired Verification binding preserves a former relationship as history but does not verify the Rule in active views.

## Verification run

A volatile observation from one execution of a Verification binding. It records the outcome and execution context without changing the durable relationship.

## ASD-STE100 finding

A structured diagnostic that identifies where descriptive text does not conform to a specified issue and rule of ASD-STE100 Simplified Technical English.

## STE dictionary import

Local data that Provenance extracts from a verified official ASD-STE100 PDF. Its identity contains the ASD issue, source digest, extracted-data digest, and extractor version. It is not canonical graph state and is not distributed with Provenance.

## Engine protocol

The versioned, language-neutral request and response contract through which an SDK invokes the Rust engine. It carries declarations, plans, queries, and verification outcomes; it never carries host-language callbacks or a mirror of Rust objects.

## Enforcement

The live path: the running code that rejects a violation. Verification is evidence about that code; enforcement is the code itself.

## Unimplemented

An active Rule with no Implementation binding. It is absence, not canonical graph state.

## Unverified

An active Rule with neither a live scanned verification site nor a canonical Verification binding. It is derived absence, not a stored status.

## Evidence site

A source line carrying a rule binding, verification binding, or provenance annotation. Its file path and line number remain its human-readable coordinate.

## Evidence anchor

The enclosing symbol and content identity recorded alongside an Evidence site's coordinate. A later scan resolves the anchor before deciding whether the site is Unchanged, New, Moved, or Gone; these states are derived report findings, not canonical graph state.

## Evidence path

A repository path the graph makes evidentiary: an Evidence site citing a known Rule, or a code path named by a Source that a Requirement references. A diff can leave the path Untouched, Touch it so re-verification is wanted, Move its durable anchor, or leave it Gone. These are report findings; running the gate performs no review or re-extraction.

## Topic

A persisted, claimable shaping work area attached to a requirement. A Topic is not a reader taxonomy classification.

## Graph reference

An immutable identification of one canonical graph scope at one pinned repository commit. Its identity includes the repository, canonical store, scope, commit, and graph content.

## Pinned commit

The complete Git commit identity from which a graph reference is read. A pinned read is independent of later working-tree changes.

## Exact export

The canonical graph content recovered for a graph reference from its pinned commit.

## Relevant canonical state

The selected scope declaration and graph records that contribute to that scope. Collaboration history and derived data are not canonical graph state.

## External correlation

An optional association between a graph reference and an identifier owned by another system. It does not participate in graph-reference identity.

## External action correlation

An optional immutable association between a Disposition and one action owned by another system. Its identity is the exact system, external scope, action kind, and stable key tuple; equal keys in different systems, scopes, or kinds are distinct. It is audit context, not Disposition identity or workflow state.

## Declaration owner

The integration URI allowed to reconcile a Source, Requirement, or Rule definition carrying the same owner. It grants no authority over other records, the whole graph, or facts the declaration does not state.

## Declaration address

An owner-local hierarchical identity for one typed declaration. Equal child keys under different parents have distinct addresses. The address is not the canonical Stable ID.

## Declaration adoption

Declaration adoption is an explicit one-time transition that assigns an unowned canonical declaration to a typed spec without changing its Stable ID or definition. It never transfers a declaration between owners.

## Retired declaration

A Source, Requirement, or Rule owned by a typed spec but omitted from that spec's next complete desired-state document. Retirement preserves the canonical record, Stable ID, owner, address, and historical relationships while removing the declaration from active graph and assurance checks. Declaring it again reactivates the same record. Retirement is not deletion.

## Commit-then-issue

The handoff in which canonical graph changes are committed before a graph reference is issued, so issuance does not create new canonical state.

## Proposal

An immutable modern candidate definition. It is always authored as `proposed`; assertion and disposition records derive its effective state without rewriting it.

## Proposal demand

A bounded occasion to consult undisposed Proposals because current work names a changed evidence path or an explicit typed Territory. Proposal demand is not a global review queue.

## Territory

The typed artifacts claimed by current shaping work: a Topic, its anchor Requirement, that Requirement's direct Domain, the Topic's Questions, and its declared artifact links. Similar names or graph proximity do not expand Territory.

## Assertion

Immutable evidence that one proposal passed unblocked adjudication using positive, uniquely owned evidence. Proposal lineage names assertion IDs, not mutable proposal state.

## Disposition

The sole immutable authority for `accepted`, `rejected`, or `deferred`. Its actor ID is a repository-allowlisted audit attestation under repository and CLI access, not proof of cryptographic or human identity.

## Ratification through action

Acceptance recorded when a human action resolves the relevant problem and produces an existing canonical artifact. The immutable Disposition names the artifact, may correlate the external action that produced it, and preserves the Proposal definition unchanged.

## Frozen legacy terminal

A pre-lifecycle proposal row whose terminal definition is covered by the compiled, versioned shipped-v1 fingerprint. It remains readable but cannot be asserted, disposed again, replaced, or used as authority for new lifecycle ingress.
