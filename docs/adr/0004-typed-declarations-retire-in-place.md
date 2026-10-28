# ADR 0004: Typed declarations retire in place

## Status

Superseded by `res_typed_declarations_use_git_history` under owner ruling D2 in
`source_owner_hygiene_rulings_20261027`.

## Decision

A typed spec is a complete desired-state document only for records carrying
that spec's owner and address prefix. When one of those Sources, Requirements,
or Rules disappears, Rust marks the canonical record retired instead of
deleting it or overloading a domain status. The record keeps its Stable ID,
owner, address, and historical relationships; active graph and assurance views
exclude it. Reintroduction clears retirement on the same record. An
identity-preserving move updates active relationships owned by the same spec.

Plan reports retirement, moves, and foreign ownership conflicts before apply.
Apply refuses conflicts. Hard deletion and ownership transfer remain separate
operations because omission is too weak a signal for either irreversible act.
