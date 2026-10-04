---
name: code-review
description: Review the changes since a fixed point (commit, branch, tag, or merge-base) along three axes — Standards, Spec, and Tests. Runs all three reviews in parallel sub-agents and reports them separately. Use when the user wants to review a branch, a PR, work-in-progress changes, or asks to "review since X".
---

Three-axis review of the diff between `HEAD` and a fixed point the user supplies:

- **Standards** — does the code conform to this repo's documented coding standards?
- **Spec** — does the code faithfully implement the originating issue / PRD / spec?
- **Tests** — which product behaviours do the tests ask the reviewer to approve, and which tests are implementation aids?

All three axes run as **parallel sub-agents** so they do not pollute each other's context. This skill then aggregates their findings.

The issue tracker should have been provided to you — run `/setup-matt-pocock-skills` if `docs/agents/issue-tracker.md` is missing.

## Process

### 1. Pin the fixed point

Whatever the user said is the fixed point — a commit SHA, branch name, tag, `main`, `HEAD~5`, etc. If they didn't specify one, ask for it.

Capture the diff command once: `git diff <fixed-point>...HEAD` (three-dot, so the comparison is against the merge-base). Also note the list of commits via `git log <fixed-point>..HEAD --oneline`.

Before going further, confirm the fixed point resolves (`git rev-parse <fixed-point>`) and the diff is non-empty. A bad ref or empty diff should fail here — not inside two parallel sub-agents.

### 2. Identify the spec source

Look for the originating spec, in this order:

1. Issue references in the commit messages (`#123`, `Closes #45`, GitLab `!67`, etc.) — fetch via the workflow in `docs/agents/issue-tracker.md`.
2. A path the user passed as an argument.
3. A PRD/spec file under `docs/`, `specs/`, or `.scratch/` matching the branch name or feature.
4. If nothing is found, ask the user where the spec is. If they say there isn't one, the **Spec** sub-agent will skip and report "no spec available".

### 3. Identify the standards sources

Anything in the repo that documents how code should be written, such as `CODING_STANDARDS.md` or `CONTRIBUTING.md`.

On top of whatever the repo documents, the Standards axis always carries the **smell baseline** below — a fixed set of Fowler code smells (_Refactoring_, ch.3) that applies even when a repo documents nothing. Two rules bind it:

- **The repo overrides.** A documented repo standard always wins; where it endorses something the baseline would flag, suppress the smell.
- **Always a judgement call.** Each smell is a labelled heuristic ("possible Feature Envy"), never a hard violation — and, like any standard here, skip anything tooling already enforces.

Each smell reads *what it is* → *how to fix*; match it against the diff:

- **Mysterious Name** — a function, variable, or type whose name doesn't reveal what it does or holds. → rename it; if no honest name comes, the design's murky.
- **Duplicated Code** — the same logic shape appears in more than one hunk or file in the change. → extract the shared shape, call it from both.
- **Feature Envy** — a method that reaches into another object's data more than its own. → move the method onto the data it envies.
- **Data Clumps** — the same few fields or params keep travelling together (a type wanting to be born). → bundle them into one type, pass that.
- **Primitive Obsession** — a primitive or string standing in for a domain concept that deserves its own type. → give the concept its own small type.
- **Repeated Switches** — the same `switch`/`if`-cascade on the same type recurs across the change. → replace with polymorphism, or one map both sites share.
- **Shotgun Surgery** — one logical change forces scattered edits across many files in the diff. → gather what changes together into one module.
- **Divergent Change** — one file or module is edited for several unrelated reasons. → split so each module changes for one reason.
- **Speculative Generality** — abstraction, parameters, or hooks added for needs the spec doesn't have. → delete it; inline back until a real need shows.
- **Message Chains** — long `a.b().c().d()` navigation the caller shouldn't depend on. → hide the walk behind one method on the first object.
- **Middle Man** — a class or function that mostly just delegates onward. → cut it, call the real target direct.
- **Refused Bequest** — a subclass or implementer that ignores or overrides most of what it inherits. → drop the inheritance, use composition.

### 4. Spawn all three sub-agents in parallel

Send a single message with three `Agent` tool calls. Use the `general-purpose` subagent for all three.

**Standards sub-agent prompt** — include:

- The full diff command and commit list.
- The list of standards-source files you found in step 3, **plus the smell baseline from step 3** pasted in full — the sub-agent has no other access to it.
- The brief: "Report — per file/hunk where relevant — (a) every place the diff violates a documented standard: cite the standard (file + the rule); and (b) any baseline smell you spot: name it and quote the hunk. Distinguish hard violations from judgement calls — documented-standard breaches can be hard, but baseline smells are always judgement calls, and a documented repo standard overrides the baseline. Skip anything tooling enforces. Under 400 words."

**Spec sub-agent prompt** — include:

- The diff command and commit list.
- The path or fetched contents of the spec.
- The brief: "Report: (a) requirements the spec asked for that are missing or partial; (b) behaviour in the diff that wasn't asked for (scope creep); (c) requirements that look implemented but where the implementation looks wrong. For each claim that behaviour has not changed, check these surfaces:
  - Which error or finding wins when an input has more than one fault.
  - Exit codes.
  - Diagnostic text and error text.
  - Output order, warning order, and finding order.
  - Write order and the state left after a failure.
  - The public Rust API of each published crate.
  - Wire names, route names, CLI names, and CLI help.
  If a refactor replaces hand-written sequences with a loop over a table, check that the PR body lists each sequence. Check that it states the order before and after the refactor. Quote the spec line for each finding. Under 400 words."

If the spec is missing, skip the Spec sub-agent and note this in the final report.

**Tests sub-agent prompt** — include:

- The full diff command and commit list.
- This brief in full:

  "Scope: every test the diff adds or changes, Rust and TypeScript, and every other test in a test file the diff touches. A test that already exists on main is not exempt.

  For each test, or group of tests that check one obligation:

  1. Class: **Spec** (bound with `#[verifies(rule, method)]` to a Rule whose statement it truly checks through a public surface: CLI command, HTTP route, public crate API) or **Implementation aid** (no Rule; it must state its reason: pins behaviour before a refactor, security hardening detail, budget, codegen check). An aid is acceptable, but it is not evidence and does not approve a behaviour.
  2. Flags, each with the test name and file:line: (1) verifies no Rule, or the bound Rule's statement is not what the test checks; (2) tests a private function or method; (3) tests a state that the code otherwise cannot reach; (4) tests non-source content (docs, guidance or instruction text, config text); (5) tests what the type system already guarantees; (6) uses a mock, fake or fixture that duplicates one elsewhere in the repo.
  3. One test, one obligation. A unit test that checks several obligations is a finding; say how to split it. Only a deliberate integration or end-to-end flow may check several steps, and its name or a one-line doc comment must say it covers a flow.
  4. For each Rule the change binds or adds: read it with `provenance <id>` and walk `provenance <id> --view grounding --depth 6`. Learn the CLI from `provenance --help`; never use `get`. Say whether it reaches the Requirement of the feature. A Rule whose natural parent sits outside the feature's subtree is a possible seam problem (code in the wrong layer, or one obligation implemented twice); name the code.

  Output, under 500 words: first **Behaviours to approve**: each Rule the change newly verifies, adds, or rewords, with its statement and the tests that verify it, so the reviewer approves behaviour knowingly. Then a table (test or group → class → Rule id or aid reason → flags). Then grounding and seam findings, or \"none\"."

### 5. Aggregate

Present the three reports under `## Standards`, `## Spec`, and `## Tests` headings, verbatim or lightly cleaned. Do **not** merge or rerank findings. The three axes are deliberately separate (see _Why three axes_).

End with a one-line summary: total findings per axis, and the worst issue _within each axis_ (if any). Do not pick a single winner across axes. The separation prevents that reranking.

## Why three axes

A change can pass one axis and fail another:

- Code that follows every standard but implements the wrong thing → **Standards pass, Spec fail.**
- Code that does exactly what the issue asked but breaks the project's conventions → **Spec pass, Standards fail.**
- Code that implements the requested feature but uses tests as evidence for a different behaviour → **Spec pass, Tests fail.**

Reporting them separately stops one axis from masking the other.
