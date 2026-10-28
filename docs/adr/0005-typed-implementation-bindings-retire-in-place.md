# ADR 0005: Typed implementation bindings retire in place

## Status

Superseded by `res_removed_implementation_bindings_are_deleted` under owner ruling D2 in
`source_owner_hygiene_rulings_20261027`.

## Decision

A typed spec treats `implementedBy` as desired state for the implementation
binding it owns. Removing the declaration marks that canonical binding retired
instead of deleting it or leaving it active. Reintroducing the relationship
clears retirement on the same binding ID, while changing the exported file or
symbol updates the same binding.

Plan presents the change on the affected Rule because the author changed what
realizes that obligation. Active assurance, documentation, and stale views
ignore retired bindings; canonical export, import, checking, and graph
references preserve them as history. Reconciliation is limited to Rules owned
by the same spec, so an equal declaration owner used by another spec does not
broaden the retirement boundary.

Deleting the binding would lose why the Rule was once considered implemented.
Leaving it active would preserve a claim the spec deliberately removed.
Retirement preserves the history without presenting it as current truth.
