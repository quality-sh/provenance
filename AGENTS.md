# Agent Instructions

This project uses **bd** (beads) for issue tracking. Run `bd onboard` to get started.

## Bootstrap

After cloning the repository or creating a worktree, enable the committed Git
hooks from that checkout:

```bash
git config core.hooksPath .githooks
```

The pre-commit hook runs formatting, Clippy, and compile checks. Fix any
reported failure before committing. When a hook must be bypassed intentionally,
use Git's standard `git commit --no-verify` option.

## Code standards

**No Rust file in this repo may exceed 500 lines.** Unreadable code is not accepted.

- New files must be designed under the limit from the start; split by responsibility, not by line count.
- Never push an existing file over the limit. If your change would, extract a module first.
- When you touch a file already over the limit, leave it smaller than you found it where practical.
- Tests count too: split large test files by the behavior they pin.

Before designing or restructuring modules, use the codebase-design and domain-modeling skills.

## Technical writing

Use ASD-STE100 Simplified Technical English, Issue 9, for technical prose in
this repository. This rule applies especially to Requirement, Rule, Resolution,
Source, and Boundary records, and to documentation, code comments, bead text,
pull requests, commit messages, and agent handoffs.

- Treat ASD-STE100 Issue 9 as the authority. Do not invent a substitute writing
  standard or add project-specific language rules in its name.
- Use descriptive writing for Requirement and Rule statements unless the text
  is an instruction that requires procedural writing.
- Run the shared Provenance statement checker before writing Requirement or Rule
  statements when that checker is available in the workflow.
- Do not claim full ASD-STE100 conformance from a clean automated report. The
  report covers only the standard rules that the checker currently implements.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
bd sync               # Sync with git
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

## Rules

A Rule is an identified atomic behavioural obligation that refines a Requirement and may
also be produced by a Resolution. It may exist before any implementation or verification.
`#[rule("rule_id")]` binds the primary production implementation to that Rule; it does
not define the Rule. For now, at most one function or type may be the primary
implementation for an id.

`#[verifies("rule_id", method)]` binds evidence to the Rule, with one method word:

- `exhaustion` — every input in a finite domain is tried
- `property` — generated inputs checked against a stated property
- `examples` — hand-picked cases
- `conformance` — an independent expression of the Rule checked against its primary
  implementation
- `construction` — a type or constraint makes violation impossible; the attribute goes on
  the type, never on a test
- `proof` — a machine-checked proof outside the test runner backs the rule; the marked
  site is the bridge pinning the implementation to the proved model

Both attributes come from `provenance-macros` (`use provenance_macros::rule;`,
`use provenance_macros::verifies;`). They expand to nothing and cost one argument check at
compile time; what they buy is a symbol the scanner finds and refactors carry along.

Unimplemented and Unverified are **absence**, derived and never stored.
`provenance coverage scan --path . --scope default --validate-rules` reports them,
along with bindings that cite unknown Rule ids and a second primary implementation
claiming one id. Verification requires a known Rule, not an implementation binding.
Adding `--strict` makes any warning a non-zero exit; how strictly CI runs the scan is a
per-repo dial, not a property of a Rule.

The Rule record's `--source-document` and `--source-section` fields cite source
material. They do not count as an implementation binding; code bindings come from
scanner-recognized attributes, helpers, decorators, or comments. They are citations, not
a planned home for the code: do not write an intended file path or symbol into them. A
Rule record holds no planned code location.

A Requirement alone is enough to anchor a Rule. `--resolution-id` is optional and belongs
only on a Rule that a Resolution really produced. Run `provenance --version` before a
session; an obsolete installed binary can demand a Resolution producer and push you into
recording one that never existed.

Rules follow behavioural obligations, not code shape. Do not mint one Rule per function,
and do not split one obligation across five Rules because the match has five arms. Prose
intent lives in the Requirement, Rule, and any Resolution that produced it. A Rule with
no function or type behind it is unimplemented—an ordinary state, not a different graph
artifact. Never write a `#[verifies]` test that asserts nothing to clear a warning.

Keep four facts independent: Rule lifecycle, decision grounding, implementation binding,
and verification binding. Lifecycle says whether the Rule is draft, under review, active,
deprecated, or archived; it does not say that code or evidence exists. Source evidence, a
Requirement, a ratified Resolution, or explicit human ratification can ground the decision.
`provenance traceability <rule_id>` reads the upstream graph chain. Implementation and
verification are separate code bindings, evaluated only by the canonical coverage scan
described above.

`prime` and `traceability` are graph reads and do not scan code. They may explain that an
active Rule is allowed to precede its implementation, but absence of a binding in those
views is not an implementation verdict. Never call a Rule invented, invalid, or unsupported
because no code implements it. This repository plans first, so a Rule can be unimplemented
until somebody writes the code. Where agent-authored behaviour has no source, Requirement,
ratified Resolution, or explicit human ratification behind it, leave the Rule `draft` or
`review`, keep the change a `proposed` proposal, or keep an open Question against it.

## Rule Doc Headers

The doc comment above a `#[rule("...")]` item is one short paragraph saying which
obligation the implementation realizes, followed only by constraints the code cannot
show for itself.
Amendment history, proof inventories, and cross-references belong in the rule's
graph record, not in the source header. `crates/provenance-cli/tests/cli_structure.rs`
enforces this mechanically: it caps how many `///` lines may sit above a
`#[rule]` attribute and fails on record-keeping phrases such as `Amended 20` and
`tracked in beads`.

<!-- BEGIN BEADS INTEGRATION -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Version-controlled: Built on Dolt with cell-level merge
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

### Auto-Sync

bd automatically syncs with git:

- Exports to `.beads/issues.jsonl` after changes (5s debounce)
- Imports from JSONL when newer (e.g., after `git pull`)
- No manual export/import needed!

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and docs/QUICKSTART.md.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

<!-- END BEADS INTEGRATION -->
