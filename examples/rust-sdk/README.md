# Rust SDK fixture

This small app keeps production code unaware of Provenance. The spec
declares traceability with `provenance-sdk`; the test follows the built
spec's handles to the `expiry` rule and verifies ordinary production
code. `provenance_spec!` checks the identifier-to-key link at compile
time and `implemented_by!` checks the implementation path exists.

From this directory, with the Rust CLI built:

```sh
../../target/debug/provenance init --path . --scope default --path-prefix .
PROVENANCE_REPO="$PWD" cargo run --bin apply
PROVENANCE_REPO="$PWD" cargo test
../../target/debug/provenance rules list --format json
../../target/debug/provenance sdk verification-runs --format json
```

`cargo run --bin apply` reconciles the spec; `cargo test` runs the
typed verification in process. The engine links in as a library, so no
engine binary or environment handshake is involved.

## Adopt existing unowned declarations

Use `adopt_unowned` only for the first typed migration of an existing record.
The method sets the explicit Stable ID and adds one exact adoption target:

```rust
let policy = source("policy")
    .adopt_unowned("source_policy")
    .document("docs/policy.md");
let document = spec("existing-requirements")
    .requirements([requirement("sharing")
        .adopt_unowned("req_sharing")
        .statement("Users can securely share documentation")
        .from(policy)])
    .build()?;
let input = document.materialize("spec://rust/existing-requirements");

let preview = operations::plan(Some(repo.clone()), &scope, input.clone())?;
assert_eq!(
    (preview.reconciliation.created, preview.reconciliation.conflicts),
    (0, 0),
);
operations::apply(Some(repo), &scope, input)?;
```

An existing record that is not a document keeps its source type only when the
declaration states that type. Use `kind(SourceType::ExternalIntegration)` in
place of `document`. `kind` gives no locator, so the canonical URL and
reference stay as they are. `document(reference)` is the short form of
`kind(SourceType::Document)` that also gives the reference.

Plan first. A valid apply keeps the Stable ID and definition and adds only the
Declaration owner and Declaration address. Richer canonical metadata outside
the typed declaration surface remains unchanged. Repeating the request is
unchanged. After adoption, use `id(existing_id)` without `adopt_unowned` for
ordinary updates.

The engine rejects missing, implicit, duplicate, malformed, and nonexistent
targets. It also rejects definition or relationship changes and records owned
by another declaration. One invalid target makes the complete apply a no-op.
