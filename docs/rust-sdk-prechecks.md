# Plan pre-check findings

Recorded 2026-08-24, before implementation work relied on either
assumption (CO-R5-9, CO-R5-10).

## CO-R5-9: Cargo artifact dependencies

Finding: artifact dependencies (bindeps) remain unstable and
nightly-only. The Cargo unstable reference requires `-Z bindeps`;
the tracking issue is rust-lang/cargo#9096. Checked against
doc.rust-lang.org/cargo/reference/unstable.html on 2026-08-24.

Result: the transport reason of the design is correct. The Rust SDK
links provenance-core and provenance-store in the same process. The
implementation does not use artifact dependencies.

## CO-R5-10: const string comparison and assertion

Finding: a `const fn` that compares `&str` bytes with a `while` loop,
asserted with `assert!` in a `const` item, compiles and holds on
stable Rust (verified on rustc 1.98.0, 2026-08-24).

Result: the `provenance_spec!` macro uses this form to connect the
identifier to the spec key. `provenance_sdk::identifier_matches_key` is
a `const fn`, and the macro uses it in a const assertion. The
compile-fail fixture `spec_key_mismatch.rs` shows that the compiler
refuses an identifier that is different from the key.
