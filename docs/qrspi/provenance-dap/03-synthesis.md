# Successor acceptance synthesis

## Scope

This synthesis binds the immutable Design revision 7, accepted fold delta, original gate
response, Ben's 2026-08-24 promotion-grace instruction, Provenance
`dc2331b98ced6f1781315f1d04df1e4ed4f83044`, and workflowd
`a80014dcc1ce38195c8bc8c0e093c159d76cd731`.

The successor changes one policy decision: G5 now permits exact graph promotion for the
successor package. It does not edit, reuse, or retro-promote the dogfood package. G1-G4,
G6-G7, CO-R5-9 through CO-R5-11, CO-R7-1 through CO-R7-7, V1 through V12, and
RR-R7-1 through RR-R7-11 keep their accepted meaning.

## Synthesis

The ownership verdict is `OwnershipReady`. The impact verdict is `ImpactReady`. The
reviewers worked independently. Both found that the engine, authoring, and CLI handler
evidence is unchanged. Both found that npm, release, CI, and dependency drift adds audit
work but does not change the accepted Rust architecture.

The successor adds two evidence updates, not new product meaning:

1. V1, V2 wording capture, and V9 baseline capture use Provenance commit `dc2331b`.
2. CAP-D10 and CAP-D12 cover the current `sqlx` 0.8, npm initializer, conditional CI,
   `CI OK`, security, and release evidence. G1 still forbids Cargo, npm, or GitHub release
   publication in this run.

No new mitigation, control, residual-risk decision, ownership choice, or Design revision
is required. The successor can proceed to one exact package gate. Approval authorizes an
exact promotion request only. It does not authorize Plan, Implementation, artifact
publication, or merge work.

## Routing

Present the exact successor package hash to Ben. After an exact approval, construct one
deterministic selection manifest. Observe the current graph before every mutation. Reuse
compatible records, fail closed on conflicts, validate the complete selection, and issue
an immutable graph reference. Then rerun Structure against that snapshot. Do not reuse
Structure R7-S1 as the authoritative successor Structure.
