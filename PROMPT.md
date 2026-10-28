Load applicable skills:
- `/test-driven-development` for features, bug fixes, and behavior changes.
- `/systematic-debugging` before fixing a failing test, stack trace, or unexpected behavior.
- `/plan` for ambiguous or multi-step work.

Claim the next ready bead: run `bd ready --json`, pick the highest-priority non-epic bead, and `bd update <id> --claim --json`.
Then run `bd show <id> --json` and read the full description, dependencies, and comments before editing.

## Project

Standalone Rust workspace for local-first Provenance — a requirements/traceability graph stored as
JSONL under `.provenance/state/` with a derived SQLite cache.

- `crates/provenance-core` — domain model (nodes, edges, validation)
- `crates/provenance-store` — JSONL state store, shards, SQLite cache, migrations
- `crates/provenance-macros` — the `#[rule]` and `#[verifies]` marker attributes
- `crates/provenance-scanner` — reads those markers, and legacy `@provenance` comments, and reports rule coverage
- `crates/provenance-cli` — CLI; integration tests in `crates/provenance-cli/tests/`

## Workflow

You MUST follow this workflow. Work one bead at a time.

### Phase 1: Understand

1. Read the bead description carefully. Read `README.md` and the relevant `docs/*.md`.
2. Read the existing code surface before changing anything, starting from
   `crates/provenance-core/src/model.rs` and the modules the bead touches.

### Phase 2: TDD Red

3. Create or extend tests before production code. CLI-visible behavior gets an integration test
   in `crates/provenance-cli/tests/`; model/store logic gets unit tests beside the code.
4. Run the narrowest relevant test and verify the new tests fail for the expected reason.

### Phase 3: TDD Green

5. Write the minimal production code to make the tests pass.
6. Run `cargo test --workspace`.
7. Run `cargo clippy --workspace --all-targets` and fix warnings.

### Phase 4: Refactor

8. Clean up names, duplication, and structure without changing behavior.
9. Run `cargo test --workspace && cargo clippy --workspace --all-targets` again.

### Phase 5: Blunt Review

10. Review your own diff as if you are Linus Torvalds reading it on LKML: API mistakes, edge
    cases, naming problems, over-engineering, write-only structures, semantic lies.
11. Pay specific attention to invariants of this repo: deterministic JSONL ordering, stable IDs,
    no volatile fields in state shards, cache is never the source of truth, edge endpoint
    validation stays exhaustive.
12. Fix every legitimate issue, add tests for missing edge cases, and re-run the gates.

### Phase 6: Commit

13. Close the bead only after implementation and verification are complete:
    `bd close <id> --reason "Completed" --json`
14. Stage only relevant files.
15. Commit with message format: `feat: <short description> (<bead_id>)` with a body explaining
    what was implemented.
16. Do not push. The drain runner controls whether pushing happens.

## Rules

- Use `bd` for issue tracking. Do not create markdown TODOs.
- No production code before failing tests for behavioral changes.
- New record types need: model struct + parse/validation, shard layout, cache materialization,
  CLI CRUD with `--format json`, and round-trip tests. Do not land a model struct alone.
- Match existing repo patterns unless the bead explicitly changes them.
- Do not modify unrelated dirty files. Do not run benchmarks.
- If you get stuck or discover follow-up work, create a linked bead:
  `bd create "<title>" -d "<context>" --deps discovered-from:<bead_id> --json`.
