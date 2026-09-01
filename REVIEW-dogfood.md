# Thermonuclear code review — `dogfood-notes` (commit cfb5025)

Scope: `git diff main...HEAD` only (11 files, +855/−3). Everything else read purely as context.
Every claimed bug below was verified end-to-end against the built binary (git 2.43.0, Linux/ext4),
not inferred from reading.

## Verification runs (both required by the review brief)

| Command | Result |
| --- | --- |
| `cargo test -p provenance-cli --features dogfood` | **exit 0** — `cli_dogfood` 9/9 passed; full per-crate suite under the feature also exit 0 |
| `cargo clippy -p provenance-cli --features dogfood --all-targets -- -D warnings` | **clean** (workspace enables clippy `pedantic` + `nursery` at warn + `-D warnings`) |
| `cargo test -p provenance-cli --test cli_dogfood_absent` (default features) | 2/2 passed — absence tests do run in the default-feature CI lane |
| `cargo build --release -p provenance-cli --features scanner` + `grep -q dogfood target/release/provenance` | no marker in the legit release binary (no false-fail); the dogfood debug binary *does* contain the marker (the check has teeth) |

---

## Constraint audit

### 1. RELEASE SAFETY — PASS today, with two structural weaknesses (F3, F4)

Cfg-gate coverage audited line by line; every gate is present and correct:

- `crates/provenance-cli/src/cli.rs:243-248` — `Command::Dogfood` variant
- `crates/provenance-cli/src/cli.rs:251-313` — `DogfoodCommand`, `DogfoodCategory`, `DogfoodSeverity`
- `crates/provenance-cli/src/handlers/mod.rs:9-10` — `#[cfg(feature = "dogfood")] mod dogfood;` (so the entire handler module, including `git_context`, is dead weight in release builds)
- `crates/provenance-cli/src/handlers/mod.rs:235-238` — the dispatch arm
- `crates/provenance-cli/tests/cli_dogfood.rs:1` (`#![cfg(feature = "dogfood")]`) and `tests/cli_dogfood_absent.rs:1` (`#![cfg(not(feature = "dogfood"))]`)
- `crates/provenance-cli/Cargo.toml:28-31` — `default = []`, `dogfood = []`; **no** other crate's feature graph references `dogfood` (verified by repo-wide grep), so no feature-unification path can pull it in transitively.
- Docs: README.md:10 and docs/release.md:6 both now say `--features scanner`; repo-wide grep finds no stale `--all-features` build instruction anywhere (AGENTS.md/PROMPT.md/skills clean).

Empirically: the `--features scanner` release binary contains no `dogfood` marker string, and a dogfood-enabled binary does — so the CI grep both passes on the legit build and would catch a leak *when the file exists*. The two ways this guarantee can rot are F3 (false-pass on missing file) and F4 (`--all-features` remains a one-flag footgun with no compile-level guard).

### 2. LOCAL-ONLY — PASS

The module's only I/O is: the local filesystem append, `std::env` reads, `/etc/hostname`, and a `git rev-parse` subprocess (dogfood.rs:277-292). `rev-parse` never touches the network, and git aliases cannot hijack it (`rev-parse` is a builtin subcommand, so alias interception does not apply). No network crate is referenced anywhere in the dogfood path. The `curl … | provenance dogfood report --enrich -` example in docs/dogfood.md:93 is operator-side piping into stdin; the CLI itself performs no network I/O and accepts no URLs.

### 3. CAPTURE NEVER FAILS ON MISSING CONTEXT — PASS for the three named cases, one gap (F7)

Verified empirically:

- All four session env vars unset → `session_id: null`, exit 0 (also covered by tests).
- Empty-string session env (`PROVENANCE_SESSION_ID=`) → filtered to null (dogfood.rs:252), verified.
- Non-git cwd → `git_context` returns `None` on both spawn failure and non-zero exit (dogfood.rs:278-291); note still written, `repo`/`branch`/`commit` null (covered by tests).
- `HOSTNAME` unset → falls back to `/etc/hostname` (verified; recorded `host` correctly), then `"unknown"`.
- `now_ms()` (dogfood.rs:255-261) is defensive and correct: pre-epoch clock → `0`, u128→i64 overflow → `i64::MAX`. No panic, no negative timestamps.

Gap: `HOME` unset is the one missing-context input that still hard-fails the capture (F7).

### 4. NO WORKFLOWD DEPENDENCY — PASS

Coupling is exactly what the design claims: four environment-variable *names* (dogfood.rs:14-19) and the enrichment JSON *shape*. Session objects are deserialized as `serde_json::Value` and passed through verbatim (dogfood.rs:47-51), so producers can add fields without a provenance release — verified: an enrichment file with an extra top-level `generated_at` field parses fine (serde ignores unknown fields), and the `contract` mismatch check is a single string compare with a clear error.

---

## Findings

### F1 · MAJOR — `branch` is null in every note, even inside a git repo

`crates/provenance-cli/src/handlers/dogfood.rs:118`

```rust
branch: git_context("--abbrev-ref=HEAD"),
```

This runs `git rev-parse --abbrev-ref=HEAD` as a **single argument**. Git parses `--abbrev-ref=HEAD` as "set abbrev-ref mode to `HEAD`" and rejects it:

```
$ git rev-parse --abbrev-ref=HEAD
fatal: unknown mode for --abbrev-ref: HEAD   # exit 128 (valid modes: strict|loose)
```

`git_context` maps any non-zero exit to `None` (dogfood.rs:281-283), so **every note ever recorded has `"branch": null`** while `repo` and `commit` populate correctly (verified end-to-end: note recorded on branch `feature/branch-x` → `"branch":null, "repo":"/tmp/…/repo", "commit":"8d2f267…"`).

The test suite cannot see this because the only branch assertions (`tests/cli_dogfood.rs:42-45`) run in a **non-git** temp dir and assert null — they'd pass even if the flag were garbage. docs/dogfood.md:39-40 explicitly promises branch stamping ("repo/branch/commit (null outside a git repo)"), so this is also silent doc/code drift on the feature's core data.

**Fix:** make `git_context` take `&[&str]` and call it as `git_context(&["--abbrev-ref", "HEAD"])`, or better `git_context(&["branch", "--show-current"])` (which yields empty → `None` on detached HEAD instead of the literal string `"HEAD"`). Add a test that `git init`s a temp repo, records a note inside it, and asserts `repo`/`branch`/`commit` are all non-null.

### F2 · MAJOR — One malformed/partial spool line permanently bricks `list` and `report`

`crates/provenance-cli/src/handlers/dogfood.rs:212-224` — `read_spool` maps each line to a `Result` and `collect()`s, so the first bad line fails the entire read, for both `list` and `report`.

Concrete verified scenario (reproduced end-to-end):

1. The spool is append-only and never compacted or validated on write completion. If `write_all` (dogfood.rs:140) dies mid-line — ENOSPC, EIO, SIGKILL — a truncated JSON fragment **without a trailing newline** stays on disk (already-written bytes are not rolled back).
2. `note` then reports failure (correct), but the spool is now armed. The next successful `note` appends onto the fragment: verified — the torn prefix and the new note become **one concatenated line**, and the new note returns exit 0 ("recorded") while being unreadable to every reader.
3. From then on: `provenance dogfood list` → exit 1, `provenance dogfood report` → exit 1, with the raw garbage line echoed into the error (dogfood.rs:222). The review channel is dead until a human hand-edits `notes.jsonl`. Same failure class for: editor/BOM accidents, an older binary reading a spool containing a future enum variant (`unknown variant`), and a reader racing a concurrent append (transient partial line; there is no locking).

60-way parallel append on local ext4 produced 60/60 intact parseable lines (Linux serializes buffered `O_APPEND` writes per syscall), so live interleaving is *unlikely locally* — the poisoning vector is torn writes and stale lines, and the reader has zero tolerance for either.

**Fix:** in `read_spool`, skip unparseable lines with a stderr warning (e.g. `eprintln!("skipping malformed dogfood note line {}: {err}", n)`), or collect `{notes, skipped}` and surface `skipped` in the report. Aggregation tools should degrade, not refuse. Optionally bound the echoed line length in the error.

### F3 · MAJOR — CI release-marker check false-passes if the binary is missing

`.github/workflows/ci.yml:43-49`

```yaml
cargo build --release -p provenance-cli --features scanner
if grep -q dogfood target/release/provenance; then
  echo 'release binary contains dogfood marker' >&2
  exit 1
fi
```

`grep -q` exits **2** on "file not found", and the `if` treats any non-zero as false — the guard is skipped, not failed. Verified:

```
$ grep -q dogfood /nonexistent; echo $?        # → 2
$ if grep -q dogfood /nonexistent; then echo FAIL; else echo PASS; fi   # → PASS
```

This is the *only* automated enforcement of the release-safety constraint, and it silently stops checking anything if `target/release/provenance` doesn't exist at that path — e.g. `CARGO_TARGET_DIR` set in the CI environment (org-level env vars, cache tooling), a future workspace layout change, or the `[[bin]]` name being renamed. The check would then pass forever while a `--all-features` release ships with dogfood compiled in.

**Fix:** make the missing binary a hard failure first:

```yaml
test -x target/release/provenance
! grep -qa dogfood target/release/provenance
```

(`! cmd` under `set -e`/default GitHub shell semantics fails the step when grep finds the marker; `test -x` fails the step when the artifact is absent.)

### F4 · MINOR — `--all-features` still compiles dogfood in; enforcement is docs + one CI lane

The feature can only leak through exactly the command the docs forbid (`cargo build --release --all-features`), and today the released artifact is "whatever the release author built", which CI never inspects — the marker check validates CI's own build, not the distributed one. Any future packaging script (Makefile, cargo-dist, brew formula) that reaches for `--all-features` ships dogfood and no CI run would notice.

**Fix (belt-and-suspenders):** make the leak a compile error instead of a process rule, e.g. in `main.rs`:

```rust
#[cfg(all(feature = "dogfood", not(debug_assertions)))]
compile_error!("dogfood is a dev-only feature; do not enable in release-profile builds");
```

Trade-off: it also blocks deliberate release-profile dogfood builds for developers; if that matters, gate via build script on `PROFILE=release` plus an explicit `PROVENANCE_ALLOW_DOGFOOD_RELEASE=1` escape hatch. Either way, also wire the marker check into whatever future release automation exists rather than only into CI's own build.

### F5 · MINOR — Empty `PROVENANCE_DOGFOOD_DIR` silently writes `notes.jsonl` into the current directory

`crates/provenance-cli/src/handlers/dogfood.rs:239-245` — `var_os` accepts the empty string; `PathBuf::from("").join("notes.jsonl")` is the *relative* path `notes.jsonl`, and `std::fs::create_dir_all("")` is a documented no-op (`Ok(())`). Verified: `PROVENANCE_DOGFOOD_DIR= provenance dogfood note …` → `recorded dogfood note in notes.jsonl`, exit 0, file created in whatever directory the agent happened to be standing in (repo pollution; the note is effectively lost from the spool).

This is inconsistent with `session_id_from_env` (dogfood.rs:252), which explicitly filters empty values — the same edge is "missing" for one env var and "cwd" for the other.

**Fix:** treat empty as unset (reuse the `.filter(|v| !v.is_empty())` pattern on the `OsString`), so an empty value falls through to `~/.provenance/dogfood`.

### F6 · MINOR — Relative `PROVENANCE_DOGFOOD_DIR` resolves per-cwd; undocumented, untested

`spool_dir()` returns the env value verbatim, so a relative override resolves against the *current working directory of each invocation*. Verified: with `PROVENANCE_DOGFOOD_DIR=relative-spool` and cwd `/tmp/x`, the spool lands at `/tmp/x/relative-spool`. Agents run from changing cwds within one session, so notes scatter across directories and `list`/`report` see different spools depending on where they were run. docs/dogfood.md:44 mentions the override but not this semantics.

**Fix:** either canonicalize (reject or absolutize relative values, ideally against the session's workspace root) or document that the override must be absolute; add a test.

### F7 · MINOR — `HOME` unset hard-fails capture

`spool_dir()` (dogfood.rs:243) propagates `skills::home_dir()`'s bail ("HOME or USERPROFILE is not set"). Session env, git context, and hostname all degrade per the design constraint — home is the one missing-context input that turns into an error. Rare (bare containers, scrubbed CI envs), and the error is clear, but it contradicts "capture must not fail for lack of context".

**Fix:** fall back to `std::env::temp_dir().join("provenance-dogfood")`, or document HOME as a hard requirement.

### Nits

- **N1 · nit** — Enrichment `sessions` map: duplicate JSON keys silently last-win (verified: `{"sessA":{"model":"first"},"sessA":{"model":"second"}}` enriches with `second`). serde/BTreeMap semantics; harmless but the contract text could say "last value wins".
- **N2 · nit** — Non-object session values pass through unchecked (verified: `"sessA": "just a string"` enriches a note with a bare string). docs/dogfood.md:62 says "fields inside each session object", implying objects; either `is_object()`-validate or document value-passthrough.
- **N3 · nit** — `dogfood list/report --format table|markdown|toon` all print pretty JSON (`src/output.rs:16-18`, shared infra). The dogfood subcommands advertise formats they cannot honor; either narrow the value_enum for these subcommands or fix `output::print` globally (pre-existing, affects every subcommand).
- **N4 · nit** — `hostname()`: `HOSTNAME` is a bash export that zsh/dash/systemd units frequently don't export, and macOS has no `/etc/hostname`, so dev Macs will record `"unknown"`. Fine for triage; a `gethostname` libc call would be exact.
- **N5 · nit** — The read_spool error embeds the entire raw line (dogfood.rs:222); a multi-MB `--detail` becomes a multi-MB error message.
- **N6 · nit** — `--surface dogfood` is accepted (dogfood.rs:229-236 derives surfaces from the command tree, which includes `Dogfood` itself in dev builds). Defensible/self-consistent; worth one line in docs or excluding the variant.
- **N7 · nit** — `--enrich -` blocks forever if stdin is an unredirected TTY, and `read_to_string` buffers arbitrary file sizes (OOM on a multi-GB file). Operator-controlled input; acceptable for a dev tool.
- **N8 · nit** — docs/dogfood.md:48 says missing context "degrades to null"; `host` degrades to the string `"unknown"`, not null.
- **N9 · nit** — No locking around concurrent note appends. Verified safe in practice on local ext4 (60/60 parallel appends intact; single `write_all` includes the newline and `O_APPEND` positions atomically), but that's a platform behavior, not a guarantee — worth a comment, not necessarily code.

---

## Test gaps (failures the current suite would NOT catch)

1. **No test records a note inside a real git repo.** This is the gap that hides F1: every `repo`/`branch`/`commit` assertion runs in a non-git dir and asserts null. A one-test fix (`git init` a tempdir, assert all three populated) would have caught the shipped branch bug immediately.
2. **No malformed/torn-line test** — F2 is invisible; nothing pins the intended behavior (fail-hard vs skip), so a fix can't be regression-checked either.
3. **No `--enrich -` (stdin) test** — the stdin branch (dogfood.rs:155-159) ships untested (verified manually working).
4. **No empty/relative `PROVENANCE_DOGFOOD_DIR` tests** — F5/F6 invisible.
5. **No quiet-flag test** — the `--quiet` suppression (verified working) is untested.
6. **No HOSTNAME-unset test** — the `/etc/hostname` fallback path is untested (testable via `env_remove("HOSTNAME")` on a Linux runner).
7. **No surface-set property test** — `note_accepts_any_known_subcommand_as_surface` checks two hard-coded names; a future refactor to a hard-coded surface list would pass every existing test. Assert instead that every clap subcommand name is accepted as a surface.
8. **The absence tests vanish exactly when they'd matter most** — under `--all-features` the `#![cfg(not(feature = "dogfood"))]` file compiles to zero tests and nothing fails. CI covers the default lane today, but nothing would notice if a future CI edit dropped it.
9. **Enrichment error paths partially tested** — wrong contract is tested; missing `sessions`, non-object root, and missing file are not.

---

## Things attacked and found sound

- **Cfg audit:** complete; no un-gated reference to any dogfood symbol anywhere (compile-verified both ways; repo-wide grep clean).
- **Local-only:** verified by full read of the module — no network types, no URLs, only `git rev-parse` + filesystem.
- **Enum serde/clap consistency:** clap `ValueEnum` and serde `rename_all = "kebab-case"` agree for every variant (all single-word), so clap values and stored JSON can't drift; the stored value is asserted by tests.
- **Validation ordering:** surface validation happens before any filesystem mutation — verified invalid surface leaves no spool file.
- **Enrichment forward-compatibility:** unknown top-level fields ignored; `contract` mismatch rejected with a message naming the expected string (tested).
- **Timestamp:** i64 ms with pre-epoch → 0 and overflow → `i64::MAX` saturation; no panic path.
- **Deterministic report:** `BTreeMap` bucketing sorts by surface, then enum declaration order (blocked < workaround < annoyance) — stable output across runs.
- **Deterministic session selection:** first non-empty in a fixed env-var priority list, documented and tested.
- **`ReportNote` flatten:** enriched and unenriched notes both serialize with an explicit `session` (null when absent) — asserted in tests.

---

## Verdict

Solid, well-gated dev tooling whose release-safety story is genuinely verified (cfg gates complete, marker check demonstrably has teeth, no false-fail on the legit build) and whose degradation paths mostly honor the "capture never fails" contract — but it ships with one silent data-loss bug (F1: the branch field has *never* worked; `git rev-parse --abbrev-ref=HEAD` is rejected by git, and the test suite structurally cannot notice because it only ever asserts branch-null in non-git directories), one robustness defect that can permanently brick the review channel off a single torn line with a follow-up note silently concatenated into unreadability (F2), and a CI guard whose `if grep` idiom silently stops guarding if the binary path ever moves (F3, exit-code-2 false-pass verified). None of the four hard constraints is violated today, so this is merge-with-follow-ups territory rather than revert territory — but F1/F2/F3 should all land before anyone starts actually depending on the notes: F1 is a two-line fix plus the missing git-repo test, F2 is a reader-tolerance fix in `read_spool`, F3 is a two-line CI hardening; F4–F7 and the nits can ride a normal follow-up. `cargo test -p provenance-cli --features dogfood` (9/9 + full workspace, exit 0) and `cargo clippy -p provenance-cli --features dogfood --all-targets -- -D warnings` (clean) both pass as of this review.
