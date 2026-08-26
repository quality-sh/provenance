# Plan pre-check findings

Recorded 2026-08-24, before implementation work relied on either
assumption (CO-R5-9, CO-R5-10).

## CO-R5-9: Cargo artifact dependencies

Finding: artifact dependencies (bindeps) remain unstable and
nightly-only. The Cargo unstable reference requires `-Z bindeps`;
the tracking issue is rust-lang/cargo#9096. Checked against
doc.rust-lang.org/cargo/reference/unstable.html on 2026-08-24.

Consequence: the design's transport rationale stands. The Rust SDK
links provenance-core and provenance-store in process; nothing in the
implementation uses artifact dependencies
(`rule_rust_plan_rechecks_artifact_dependencies`).

## CO-R5-10: const string comparison and assertion

Finding: a `const fn` that compares `&str` bytes with a `while` loop,
asserted with `assert!` in a `const` item, compiles and holds on
stable Rust (verified on rustc 1.98.0, 2026-08-24).

Consequence: the `provenance_spec!` identifier-to-key link uses this
form: `provenance_sdk::identifier_matches_key` is const, and the macro
pins the link with a const assertion. The compile-fail fixture
`spec_key_mismatch.rs` keeps the refusal pinned
(`rule_rust_plan_rechecks_const_string_asserts`).
