# ADR 0001: Immutable proposal lifecycle and local attestation

## Status

Accepted.

## Decision

Proposal definitions, assertions, and dispositions are immutable. Definitions always store
`proposed`; effective `asserted`, `accepted`, `rejected`, and `deferred` state is projected
from assertion and disposition records. Lineage names assertion IDs. One aggregate validator
guards direct writes, swarm landing, import, repository checking, and materialization.

Modern disposition actor IDs must be present in the repository manifest allowlist. Frozen
historical dispositions are exempt because their exact authority is established by the compiled
audit fingerprint. Version 1 trusts repository and CLI access and does not claim cryptographic
signatures, account authentication, or proof that a human controlled the supplied ID.

Persisted terminal proposal rows shipped before this lifecycle remain compatibility input.
Their exact set and historical disposition audit are frozen by compiled, versioned `ShippedV1`
fingerprints; editable repository data cannot grant compatibility. The policy-aware aggregate
validator checks both fingerprints, embedded terminal state stays authoritative, and the rows
cannot re-enter the lifecycle. New or changed embedded terminal state fails closed.

Qualifying swarm proposals carry immutable assertions; qualification uses the complete aggregate,
including synthesis-only follow-ups. Missing or invalid assertion evidence rejects the whole
journal batch, and swarm output cannot supply disposition authority.
Whole-scope import stages and checks the complete state
directory before publication. A repository publication lock precedes lifecycle and shard locks;
a durable phase marker restores backup or finishes cleanup after interruption. Publication uses
portable directory renames rather than claiming an unavailable atomic directory exchange.
Graph-reference projection remains limited to canonical
graph families; lifecycle records do not affect exact export or digest identity. Proposal
consultation remains demand-driven by normalized path or typed territory and is not added to Prime.
Disposition audit may carry one closed external-action tuple (system, external scope, action
kind, stable key), while canonical-artifact references resolve against the exact local scope,
artifact kind, and stable ID at every persistence and validation seam. Absent additive audit
fields preserve the frozen shipped-v1 serialization and fingerprints.
