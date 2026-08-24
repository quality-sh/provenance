# Successor impact-and-risk review

## Exact scope

- Subject: Design revision 7, accepted fold delta R7, gate response R7, and Structure R7.
- Original accepted package:
  `9eded2d99fc6cf4778fda9ab3d7e3345a40a15f5c578afb49b826c189aef6463`.
- Reviewed Design:
  `b0df0aad0b2a63a2f9a28be705aa44edc21c438c04f736acfdc84cdc047d3e3d`.
- Accepted fold delta:
  `9663a16c16a763c74eb6560504c7d248f49aaad7b90a951873b466b5e4086445`.
- Gate response:
  `88d3c6b9cfbdab73fb75bf83c26d6d6e03caa21ab8c45580ff37f652ac90bd69`.
- Successor policy delta: G5 permits promotion in the successor run only. Original
  records remain immutable. G1-G4 and G6-G7, CO-R5-9 through CO-R5-11, CO-R7-1 through
  CO-R7-7, and RR-R7-1 through RR-R7-11 retain their accepted meaning.
- Repository evidence: Provenance `origin/main` at
  `dc2331b98ced6f1781315f1d04df1e4ed4f83044`, compared with
  `11ebe84cbbf343b8cf37766340346940bdb706d6`.
- Promotion contract: workflowd
  `a80014dcc1ce38195c8bc8c0e093c159d76cd731`.
- Independence: no ownership report or conclusion was read or used. Inspection was
  read-only.

The original package and Structure remain non-promotable history. The successor must
create its own acceptance scope, package, gate response, promotion request, result, and
graph snapshot. Structure R7 can inform the new projection but cannot serve as its
snapshot-bound Structure input.

## Affected surfaces

- The core design evidence is unchanged. The Git tree objects for `provenance-core`,
  `provenance-store`, `provenance-scanner`, `provenance-ste100`, and
  `provenance-cli/src/handlers` match the original pin.
- The cited TypeScript authoring files also match: declarations, materialization, spec,
  fluent types, implementation references, and verification orchestration.
- Cargo dependency state changed: `sqlx` moved from 0.7 with
  `runtime-tokio-rustls` to 0.8 with `runtime-tokio`. This affects store dependency,
  MSRV, and publication-audit evidence, but not the accepted ownership or API design.
- npm distribution expanded with `@quality-sh/create-provenance`, Yarn Plug'n'Play
  support, platform-package metadata, and revised engine-install guidance.
- CI and release workflows now use conditional path routing, a consolidated `CI OK`
  result, security jobs, pinned actions, trusted npm publishing, and initializer
  publication.
- CLI changes are confined to complete skill-directory copying. SDK handlers and the
  operations that DR4 moves remain unchanged.
- The graph now contains related distribution decisions, including
  `res_sdk_engine_from_package_manager` and `res_typed_facade_owns_construction`. They
  are compatible context. Promotion must observe and link or refine these records, not
  create unexamined duplicates.

## Controls

- CO-R7-1 through CO-R7-7 remain sufficient for check partitioning, call placement,
  first-error order, wording, trimming, and discovery fallback.
- V1 through V12 remain sufficient for behavior preservation, no-write refusal,
  identity, serialization, facade limits, verification failure handling, purity, and
  cross-frontend parity. Baseline captures must use successor pin `dc2331b`, including
  V1, V2 wording, and V9 goldens.
- DR13/C1 still prevents a second materializer, write path, identity owner, protocol
  constant, or semantic layer.
- DR12/V6 must add the Rust example job to current conditional CI routing and to the
  `CI OK` dependency set.
- The workflowd exact-promotion seam remains mandatory: complete deterministic selection,
  authoritative pre-mutation observation, conflict handling, validation, and an immutable
  snapshot bound to the successor scope.
- G1 remains sufficient. Graph promotion is not Cargo, npm, or GitHub release publication.
  G1 and RR-R7-6 must themselves be promoted as controls. No artifact publication may
  occur in this run.
- The later DR10 audit must use the current dependency and release evidence, including
  `sqlx` 0.8, facade dependencies, MSRV, public API exposure, `engine_version`, and shared
  Cargo/npm version and tag coupling. This does not bring npm initializer work into the
  Rust implementation scope.

## Residual risks

- RR-R7-1 through RR-R7-5 retain their accepted controls; authoring and engine semantic
  sources did not drift.
- RR-R7-6 has more release evidence to audit, but G1 still blocks the hazardous action
  before a separate human decision.
- RR-R7-7 through RR-R7-10 are unchanged.
- RR-R7-11 remains accepted with V1-V12 and the Plan-entry checks, using `dc2331b` as the
  execution baseline.
- Existing graph records can cause identity or content conflicts during promotion. The
  pinned promotion contract fails closed and routes conflicts to its gate; this
  uncertainty does not require a new design decision.
- Any change to accepted meaning, ownership, control, or residual-risk disposition after
  this review requires a new review. Evidence-only links do not.

## Findings

1. No repository drift changes the accepted architecture, validation partition, stateful
   boundary, identity ownership, or verification model.
2. Packaging, release, dependency, and CI drift increases audit work but stays within G1,
   CAP-D10, DR12, and V6/V8.
3. Promotion is safe only as a successor operation. Reusing or editing the dogfood
   package, gate response, or Structure would violate immutable history and the pinned
   workflowd contract.
4. The accepted controls and residual-risk dispositions remain sufficient for graph
   promotion through a newly scoped, authoritatively observed successor request.

## Verdict

`ImpactReady`
