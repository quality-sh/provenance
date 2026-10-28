# ADR 0006: Typed verification bindings retire in place

## Status

Superseded by `res_replaced_verification_bindings_are_deleted` under owner ruling D2 in
`source_owner_hygiene_rulings_20261027`.

## Context

ADR 0003 makes the verification owner, canonical Rule, and an owner-local key
the stable identity of a verification relationship. Nothing retired that
relationship. A test that once called `verify` and later stopped left an active
binding behind, so the Rule kept presenting as verified after the relationship
disappeared.

A verification run is not a desired-state document. One execution sees the call
sites it happened to run, not every call site in the repository, so absence
from a run is too weak a signal to retire anything.

## Decision

A run is desired state only for the owner, file, and key it reports. When a
verification owner reports one of its keys in a file against a different Rule,
Rust marks the binding that key previously named retired instead of deleting it
or leaving it active. The record keeps its Stable ID, owner, key, and history,
and reporting it again clears retirement on the same ID. Only the owner named
in `declared_by` retires through this path, and the same key reported from
another file is a separate relationship that stays untouched.

Active coverage, wiki, stale evidence, and plan views exclude retired bindings.
Export, import, checking, and exact graph references preserve them. Bindings
whose recorded file no longer exists are surfaced by the stale gate rather than
retired, because a run that never executed the file cannot vouch for it.

## Consequences

A rewired test retires the relationship it replaced without losing why the Rule
was once considered verified. A filtered or sharded run cannot mass-retire
relationships it did not observe, and no single run can retire another owner's
work. Hard deletion remains a separate operation.
