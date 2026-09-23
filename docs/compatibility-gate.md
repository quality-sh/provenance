# Compatibility gate

The gate enforces Decision 5 of the API restructure (docs/api-contract-v2.md,
section 8): agents never bump a version, a release, or a compatibility value
without express human authorization. A human writes the authorization. The gate
checks it. The gate is bead provenance-if20.

## What the gate watches

The gate reads one set of watched values at two revisions, the base and the
head of a change. It fails when any watched value differs and the change
carries no valid authorization.

| Axis | Source |
| --- | --- |
| wire (SDK_PROTOCOL_VERSION) | crates/provenance-core/src/protocol.rs |
| state (SUPPORTED_SCHEMA_VERSION) | crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs |
| review_journal (REVIEW_SCHEMA_VERSION) | crates/provenance-core/src/review.rs |
| read_derivation (READ_DERIVATION) | crates/provenance-store/src/operations/stamp.rs |
| state mirror (STATE_SCHEMA_VERSION) | packages/provenance/src/protocol.ts |
| workspace crate version | Cargo.toml (`[workspace.package] version`) |
| npm package versions | packages/provenance/package.json, packages/create-provenance/package.json |

These are today's source locations for the four compatibility-tuple members
plus the released package versions. Phase 2 collapses the four members into one
tuple definition; when that happens, this table shrinks to the one definition
and the gate keeps its behavior.

## The AUTHORIZED-BUMP marker

`AUTHORIZED-BUMP` is a file at the repository root. Only a human creates it.
Agents must never create it or edit it. The marker authorizes one change. The
format:

```
# Human authorization for one compatibility change. Agents never write here.
authorized-by: Ben
date: 2026-09-13
pull-request: 280
commit: 61948751dbbf818ed49b2e16b6ea048de7c8b17b
reason: the restructured API replaces the versioned operation URL
```

The gate requires these five fields. The date is a real calendar date in
YYYY-MM-DD form. The `pull-request` field names the pull request that carries
the change. The `commit` field names a commit of that change; the SHA of the
bump commit is the usual choice. The reason names why the break is authorized.

The marker's own text proves nothing, so the gate cross-checks the binding
against the event context: the pull request must match the pull request under
review, the named commit must be one of the change's own commits, and the
marker date must be on or after the base commit's date. A marker copied from
an earlier change fails all three — its pull request differs, its commit is
outside this change, and its date predates the base. This is a detectable
proxy, not a proof of authorship: a human still writes the file, and the
binding makes every earlier marker unusable as-is. Pushes to main carry no
pull-request context, so the full binding is checked on pull requests; the
format and freshness rules apply everywhere.

## Freshness rule

A marker authorizes the change that carries it. The gate compares the marker at
the base revision with the marker at the head. When a watched value changed and
the marker text is unchanged from the base, the gate fails. A marker left in
the tree authorizes nothing later. Every bump needs a new marker, written in
the same change as the bump. The binding above closes the remaining gap: when
the marker file is absent at the base, re-adding a historical marker still
fails, because that marker names a commit outside this change, a pull request
that is not under review, and a date before the base commit existed.

## Where the gate runs

The `Compatibility gate` workflow (.github/workflows/compatibility-gate.yml)
runs on every pull request and on every push to main. It compares the head with
the pull-request base, or with the previous head on a push. The job needs no
Rust toolchain and finishes in seconds; it also runs the gate's tests.

Run the gate locally before you open a change:

```sh
node tools/compat-gate/check.mjs                 # base = merge-base with origin/main
node tools/compat-gate/check.mjs --base <sha>    # explicit base
node --test tools/compat-gate/check.test.mjs    # gate unit and integration tests
```

## What the gate does not do

The gate does not judge the content of a change. It does not stop a release;
the release flow has its own checks. A passed gate says one thing: a human
wrote a fresh authorization for this compatibility change.
