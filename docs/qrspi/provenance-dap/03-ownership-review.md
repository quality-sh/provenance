# Successor semantic-ownership review

## Exact scope

- Base meaning: immutable Design revision 7 package
  `9eded2d99fc6cf4778fda9ab3d7e3345a40a15f5c578afb49b826c189aef6463`,
  including its accepted folds and G1-G7 decisions.
- Successor change: G5 permits promotion. G1-G4 and G6-G7 remain unchanged.
- The original package, gate response, and Structure R7-S1 remain non-promotable
  history. Promotion can apply only to the successor acceptance scope.
- Repository evidence: Provenance
  `dc2331b98ced6f1781315f1d04df1e4ed4f83044`; workflowd
  `a80014dcc1ce38195c8bc8c0e093c159d76cd731`.
- Review subject: the named Design, fold, gate-response, and Structure sections only.
  No impact-review material was read or used.

## Evidence

The semantic engine trees have identical Git identities at `11ebe84` and `dc2331b`:

- `provenance-core`: `1ea518aa13df819b84d18eda62aab57f792c02ed`
- `provenance-store`: `0adb3fd9b4e368f76eb1fbd4ce2401815a2fc422`
- `provenance-scanner`: `007c574d7a850a101ac1204fdef04faf43dfd98b`
- `provenance-ste100`: `a20ac3473a387b83df4e4e427b8921b3aa9c4fe0`

The TypeScript authoring, materialization, handle, implementation-reference, and
verification sources are unchanged. The CLI SDK handler tree is unchanged.

The changed evidence consists of npm engine packaging, error guidance, Yarn Plug'n'Play
support, and the new TypeScript initializer; conditional CI routing, a required `CI OK`
gate, security jobs, and platform install tests; release hardening and initializer
publication; CLI skill-directory copying and committed-copy checks; the `sqlx` 0.8
dependency update; and graph mappings for existing SDK and typed-declaration meaning.

## Findings

- Core remains the correct owner of the pure authoring kernel, document-decidable
  structural checks, address construction, canonical authored assembly, and input
  protocol types.
- Store remains the sole owner of identity, migration, reconciliation, ownership refusal,
  STE write gates, locking, planning, and mutation. Scanner and STE ownership remain
  unchanged.
- npm and initializer changes stay within frontend distribution ownership. The
  initializer invokes the installed Rust engine to initialize and validate state; it
  does not create a second semantic path.
- CI and release changes do not move product semantics. CAP-D12 must join the current
  path filters and `CI OK` dependency set. CAP-D10 already owns the updated dependency
  and release audit.
- CLI skill changes are separate from the unchanged SDK handlers and do not affect the
  kernel/store seam.
- Current graph records are compatible. `res_typed_facade_owns_construction` assigns
  TypeScript its host-language builders and handles; it does not assign engine validation,
  identity, or mutation to TypeScript. `res_sdk_engine_from_package_manager` governs
  non-Rust frontend distribution; G2 governs Rust through Cargo semver and in-process
  calls.
- No accepted semantic item is unowned, and no current graph record creates a conflicting
  owner.
- The successor must receive its own promotion-policy identity, acceptance records,
  promotion result, and graph snapshot. It must not amend or reuse the original dogfood
  authority records.

## Verdict

`OwnershipReady`
