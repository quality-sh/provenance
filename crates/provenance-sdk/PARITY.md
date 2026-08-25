# Rust and TypeScript SDK parity ledger

This ledger records where the Rust SDK and the TypeScript SDK differ.
Every entry falls in one of three delta classes.

## Class 1: ordering and collation

- CLOSED (dissolved). The kernel orders kernel-authored documents by
  UTF-8 byte order; the TypeScript SDK's ICU collation is a redundant
  local mirror. Wire-received documents are never reordered by either
  side, so no observable delta remains.

## Class 2: type projection

- TypeScript projects literal-key unions with compile-time key safety
  over its handles. Rust projects `provenance_spec!`, which pins the
  identifier-to-key link with a const assertion, over the kernel's
  string-keyed handles. Unknown keys fail at compile time in
  TypeScript and at runtime in Rust, except the spec key itself, which
  `provenance_spec!` checks at compile time.
- The 13 TypeScript type fixtures map as follows: the spec-key fixture
  maps to the `provenance_spec!` compile-fail case; the frozen-instance
  fixtures map to the by-value builder and the immutable
  `SpecDocument`; the remaining literal-key fixtures fall in this
  class as runtime checks on `handles()`.
- `implemented_by!` checks path existence with `include_bytes!`;
  TypeScript resolves the symbol with the compiler API. A missing file
  fails both at compile time; a missing symbol fails only TypeScript
  at compile time.

## Class 3: capability

- Rust `adopt_unowned(existing_id)` and TypeScript
  `adoptUnowned(existingId)` produce the same protocol 5 exact allowlist. Both
  use the Rust ownership decision. Rust links it in process; TypeScript checks
  the engine protocol before it sends the document.

- Verification is synchronous in Rust v1; asynchronous callbacks are a
  gate-visible follow-up (G7).
- TypeScript `verify` takes `options.method` (default `examples`) and
  `options.symbol`; Rust `verify` fixes the method to `examples` and
  records no symbol in v1.
- TypeScript records `error.stack` for a failing callback and rethrows
  the original error; Rust records the error's display text and returns
  a new error carrying that text, so the original error type and source
  chain do not propagate.
- `PROVENANCE_BIN` and the runtime engine handshake do not apply: the
  Rust SDK links the engine in process, and compatibility is cargo
  semver on provenance-core and provenance-store (G2).
- With `panic = "abort"` a Rust verification unwind never reaches the
  failure recorder; TypeScript has no equivalent mode.
