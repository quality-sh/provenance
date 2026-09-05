# CLI

The `provenance` command comes from a release archive, `cargo install
provenance-cli`, or the `@quality-sh/provenance` development dependency. The
native distribution also includes `cargo-provenance`, which Cargo runs for
`cargo provenance`.

For Rust, run `cargo install provenance-cli` and then `cargo provenance init`.
The initializer finds the current Cargo workspace and adds `provenance-sdk` at
the running CLI version. Use `--package <name>` when the workspace contains
multiple packages. For TypeScript, run `npx --yes
@quality-sh/create-provenance`. The initializer supports npm, pnpm, Yarn, Bun,
Deno, and Nub.

Common local workflow:

```sh
provenance init --path . --scope default --path-prefix . --disposition-actor-id reviewer
provenance sources create --scope default --id source_policy --name "Policy"
provenance domains create --scope default --id domain_policy --name "Policy"
provenance requirements create --scope default --id req_policy --statement "Follow policy" --domain-id domain_policy
provenance requirements source-ref add --scope default --requirement-id req_policy --source-id source_policy
provenance materialize --format json
provenance export --scope default --format json --output provenance-export.json
provenance check --format json
```

Agent-facing commands support JSON output for deterministic parsing.

`provenance check --format json` reports `diagnostics` for new Requirement and
Rule records and for records whose statement differs from Git HEAD. These
ASD-STE100 findings are informational, so the command still exits successfully.
The array is empty when no Git HEAD is available. This reporting contract does
not set a repository-wide STE enforcement policy.

`provenance check --strict --format json` selects Git HEAD as the committed
candidate. By default, it compares that candidate with its first parent. It
uses an empty base when HEAD is the initial commit. Use `--base <commit>` to
select a different base, such as the target branch commit for a pull request.
The check analyzes only Requirement and Rule records that are new or whose
statement bytes changed between the two commits. Thus, unchanged findings in
permitted history do not fail the check.

The checkout must contain the selected base commit. A shallow checkout that
omits the first parent fails with a request for more Git history. It does not
treat an unavailable parent as the empty initial-commit base.

Strict statement checking uses the same Rust checker as statement writes. It
also uses the project dictionary reference when the referenced local index is
available. The command prints the complete JSON report to standard output
before it exits nonzero for findings. The report identifies the resolved
`candidate_commit` and `base_commit`. A null `base_commit` identifies the empty
base for an initial commit. Direct edits to canonical JSONL files do not bypass
this committed-candidate check.

Strict mode reads candidate statements from Git commits. It resolves the
project dictionary reference and local index from the working tree. Thus, a
dirty dictionary reference can change vocabulary findings, but dirty statement
bytes cannot change the committed candidate.

The `--strict` option for `provenance check` applies only to ASD-STE100 findings
in committed Requirement and Rule statements. The `--strict` option for
`provenance coverage scan` applies to Rule binding and coverage warnings. These
options enforce different reports.

## Typed SDK protocol (POC)

Rust consumers do not need these one-shot commands. The `provenance-sdk` crate
provides the authoring builders, operations, `verify`, and the `rule` and
`verifies` attributes. See `examples/rust-sdk`.

The SDK protocol uses one-shot commands that read or write JSON:

```sh
printf '%s' '{"statement":"Install the cover."}' | provenance sdk check-statement --format json
provenance sdk apply --repo . --scope default --format json < declarations.json
provenance sdk begin-verification --repo . --scope default --format json < begin.json
provenance sdk complete-verification --repo . --scope default --format json < complete.json
provenance sdk verification-runs --repo . --scope default --rule <canonical-rule-id> --format json
```

`check-statement` accepts exactly one object with a string `statement` field. It
does not resolve a repository. It writes the authoritative
`provenance-ste100::check_descriptive` Report unchanged, including UTF-8 byte
spans, so a later editor adapter can transport the result without implementing
ASD-STE100 rules.

`apply` creates missing records and updates records whose `declared_by` matches
the document. It refuses implicit takeover and foreign-owned collisions before
writing. Protocol 5 also accepts an exact, per-target `adopt_unowned` allowlist.
An adoption target must name one declaration with the same explicit Stable ID,
and its definition and relationships must already match canonical state.
Omitted owned declarations retire in place. Identity-preserving moves replace
their active owned relationships. `plan` previews creates, updates, moves,
adoptions, retirements, and ownership conflicts without writing. Verification runs
live in the derived cache and always cite an existing Rule. Begin accepts either
a canonical Rule ID or a declaration owner plus hierarchical address;
the language callback itself runs in Node and never crosses into Rust. See
[`typescript-sdk-poc.md`](typescript-sdk-poc.md) for the package interface,
identity rules, limits, and experiment results.

## Structured engine queries

Eight read-only commands answer the questions an agent asks between a plan and
an apply. Each one is a named operation that reads a JSON request on stdin and
writes one bounded JSON answer, so nothing here is a query language and nothing
needs a daemon:

```sh
printf '%s' '{"node_type":"rule","id":"<rule-id>"}' | provenance sdk get --repo . --scope default --format json
printf '%s' '{"text":"time bounded","limit":20}' | provenance sdk search --repo . --scope default --format json
printf '%s' '{"id":"<rule-id>","direction":"in"}' | provenance sdk neighbors --repo . --scope default --format json
printf '%s' '{"id":"<source-id>","direction":"out","max_depth":2}' | provenance sdk trace --repo . --scope default --format json
printf '%s' '{"id":"<requirement-id>"}' | provenance sdk impact --repo . --scope default --format json
printf '%s' '{"rule":"<rule-id>","base":"<commit>"}' | provenance sdk evidence --repo . --scope default --format json
printf '%s' '{"base":"<commit>","rules":["<rule-id>"]}' | provenance sdk stale --repo . --scope default --format json
printf '%s' '{"file":"src/share-links.ts","symbol":"createShareLink"}' | provenance sdk resolve-symbol --repo . --scope default --format json
```

Every answer opens with `protocol_version` and `operation`, so a recorded
response says which contract produced it. `sdk info` still reports the version
the engine speaks; a request may name `protocol_version` itself, and the engine
refuses a request written for another one. Every request accepts
`include_retired`, false by default: active views leave retired records and
retired bindings out, and this flag is the only way to see them. Every request
that can match more than one record accepts `limit`, 50 by default and 200 at
most, and its answer carries `limit` and `has_more`. An answer stops at the
limit; there is no cursor and no next page.

Every answer carries a `stamp` that says what it reflects. `serial` and
`digest` name the projection revision the rows came from and `instance_id` the
projection instance; serials compare only within one instance. `derivation` is
the reader logic version, which moves when the same rows answer differently.
`policy` is the freshness step the reader ran: `catch_up`, which brings the
projection up to date under the publication lock before the read;
`annotate_only`, which answers at the stored revision; or `catch_up_failed`,
when the step refused and the answer is at the stored revision with the error
text in `freshness_error`. The commands run `catch_up`; no flag selects a
policy yet, and the `read.freshness_policy` setting is reserved for that. `attested` names the projection tables behind the
answer. `live` names what the stamp does not cover, from a closed list:
`canonical` (canonical shards), `scanned_sites` (a scan of the working tree),
`verification_runs` (the run file), and `diff` (git). A stamp never implies
freshness for anything it does not list.

`get` takes `node_type` and `id` and answers `found` plus the canonical record
under `node`, tagged with the same `node_type`. The node kinds are `source`,
`requirement`, `resolution`, `rule`, `topic`, `question`, `domain`, and
`boundary`. `search` takes `text`, an optional `node_types` filter, and answers
`nodes`: records whose id, statement, name, description, title, or question
contains the phrase. A search that names no kinds answers the six original
kinds; `domain` and `boundary` records are answered only when `node_types`
names them.

`neighbors` takes `id`, an optional `node_type`, a `direction` of `out`, `in`,
or `both`, and an optional `relations` filter naming declared relations
(`cites`, `domain_id`, `refines`, `depends_on`, `supersedes`, `spawned_by`,
`requirement_ids`, `resolution_ids`, `requirement_id`, `topic_id`,
`resolution_id`, `contradicts`, `links`); an unknown name is refused. It reads
every declared relation one hop away and answers `neighbors`, each carrying
the `relation`, the `direction` it was read in, and the record at the other
end. A question owns `contradicts`, so the `neighbors` of a requirement answer
the question `in`, and the other requirement is one more hop away. `out` is a
relation the record holds in its own field; `in` is a relation another
record's field holds toward it. A Rule's `out` neighbours are the Requirements
and Resolutions its lists name; a Requirement's `out` neighbours are its
Sources and its domain, and its `in` neighbours the Rules and Resolutions that
name it. `trace` takes the same parameters plus `max_depth`, 3 by default and
10 at most, and answers `nodes`, each carrying the `depth` it was reached at.
Tracing `in` from a Source reaches the Requirements that cite it at depth 1
and the Rules that name them at depth 2; tracing `out` from a Rule reaches its
Requirements at depth 1 and their Sources at depth 2.

`impact` takes `id` and answers `affected_rules`: every Rule the record
reaches, each with the `implementations` and `verifications` that stand behind
it, in the same shape `plan` already reports. The walk follows each declared
relation in its flow direction and never adds a step no declaration gives: a
resolution reaches the rules that name it, not the requirements it answers.
The scan of the working tree behind the sites stops at a configured file count
over a sorted walk, and `scan_cut` is true when it stopped, so the scanned
sites are then a lower bound. `resolve-symbol` takes `file` and an optional
`symbol` or `line` and answers `rules`, the Rule records bound to that code
site. It scans the named file alone, so the file count never applies to it.

`evidence` takes `rule` and answers what stands behind it, kept apart by kind:
`implementation_bindings`, `verification_bindings`, `verification_runs`,
`latest_verification_run`, `review_required` with the `reviews` that raised it,
and `stale`. Review required means the Requirement the Rule serves was
restated; stale means the code carrying the evidence changed. Stale is read
from a diff and never guessed, so `stale` is null unless the request names a
`base` commit, with `head` defaulting to the current commit. Each of the four
lists carries its own cut flag, `implementation_bindings_has_more`,
`verification_bindings_has_more`, `verification_runs_has_more`, and
`reviews_has_more`; the top-level `has_more` is true when any of them is.

`stale` takes `base`, an optional `head`, and an optional `rules` filter, and
answers the disturbed evidence `sites` with a `summary` counting them. It uses
the same `touched`, `moved`, and `gone` words as [the diff evidence
gate](#diff-evidence-gate), and reports the same sites for a named set of Rules
instead of the whole graph.

## Rule coverage

A Rule's record states the behavioural obligation; code bindings name the Rule back. In
Rust, `#[rule("rule_id")]` marks its primary production implementation, and
`#[verifies("rule_id", method)]` marks evidence. A Rule may exist before either binding.
The methods are
`exhaustion`, `property`, `examples`, `conformance`, `construction`, and `proof`. A
marked type uses `construction`, because building the value is the proof. A test whose
expected values come from a machine-checked model (a Lean theorem, for example) uses
`proof`: the proof checker runs in your CI, and the marked test is the bridge between
the proved model and the implementation.

```sh
provenance coverage scan --path . --scope default --validate-rules
provenance coverage scan --path . --scope default --validate-rules --strict --format json
provenance coverage scan --path . --format json --output coverage.json
provenance coverage scan --path . --baseline coverage.json --validate-rules --format json
```

Without `--validate-rules` the scan only reports what it found in the tree. With it, the
scan loads the scope's Rules and warns about a binding that cites an unknown Rule, a
second primary implementation for one Rule, and an active Rule with no implementation
or verification site anywhere. Verification needs a known Rule, not an implementation
binding. Unimplemented and Unverified are derived at scan time and never stored, so no
shard can disagree with the code.

The scan derives those absence findings only when `--path` names the repository root.
A narrower scan still validates every binding it encounters, including unknown Rule ids
and duplicate primary implementations, but it cannot claim that a binding is absent from
the rest of the repository.

`--strict` exits non-zero when the report holds any warning; the report still prints
first. That is the dial each repository sets for itself: strict in CI once a repository
wants every active rule verified, plain while it is still filling them in.

Each annotation and binding in a scan report keeps `file_path` and `line` and adds a
durable `anchor`: the enclosing symbol plus a SHA-256 hash of the cited line's trimmed
text. Pass an earlier JSON scan with `--baseline` to resolve those anchors. An
`unchanged` site is pinned to a baseline site, a `moved` site reports its new `line`
and `original_line`, and a `gone` site retains its last coordinate. A `new` site has
no baseline site sharing its anchor; without `--baseline` the scan has nothing to
compare against, so every site is `new`. With `--validate-rules`, gone anchors
produce warnings; moved anchors do not become false absence warnings.

Anchors relocate across files. A site missing from its own file is matched by rule,
symbol, and line hash against baseline-unaccounted sites anywhere in the scan before
anything is declared missing: a single match is `moved`, reporting `original_file_path`
alongside `original_line`; no match is `gone`. Several matches make the scan warn,
naming every candidate, rather than pick one. When identical sites share one anchor,
each is pinned to a baseline line where it can be, and a lost instance is reported
gone; when the survivors cannot be told apart, they stay at their current coordinates
as unchanged and the scan warns that the group lost instances. Identical sites
shuffled within one file with none lost stay silent.

## Diff evidence gate

`stale` is the read-only answer to “does this diff intersect evidence in the graph?” Give
it either two commits or one `--since` commit (whose other endpoint is `HEAD`):

```sh
provenance stale main HEAD
provenance stale --since main --format json
provenance stale --since main --strict
```

The report includes every binding or annotation that cites a known rule, every
verification site, and every repository path named by a Source that a Requirement
references. Each site is `untouched`, `touched`, `moved`, or `gone`. Touched means the
diff intersects that site's lines and re-verification is wanted. Moved and gone are
resolved through the coverage scanner's durable symbol and content-hash anchors; a pure
relocation is moved rather than touched. Source references carrying explicit line ranges
are intersected at those lines; a path without lines is touched by any edit to that file.

Plain mode always reports and exits zero. `--strict` prints the same report, then exits
non-zero when any site is touched or gone. The command performs no review-trigger firing,
agent work, requirement extraction, or state write.

The scanner is line-oriented. Its native binding patterns and current limits are:

| Language | Primary implementation binding | Verification binding | Recognition grade |
| --- | --- | --- | --- |
| Rust | `#[rule("id")]` | `#[verifies("id", method)]` | Binding-grade for both |
| TypeScript | `const name = rule("id", fn)` | `verifies("id", "method")` inside a named function or function-valued `const` | Binding-grade for both |
| JavaScript | `const name = rule("id", fn)` | `verifies("id", "method")` inside a named function or function-valued `const` | Binding-grade for both |
| Python | `@rule("id")` above `def` | Comment channel | Binding-grade rule; comment-only verification |
| Go | `var name = rule("id", func...)` | Comment channel | Binding-grade rule; comment-only verification |
| Java | assigned `rule("id", lambda)` static-helper call | Comment channel | Binding-grade rule; comment-only verification |

Keep the helper name, opening parenthesis, and quoted id on one line. An assignment may
start on that line or, for the Java-style field layout, immediately above it. Copyable
identity-helper implementations and their exact constraints are in
[`rule-bindings.md`](rule-bindings.md). The installed TypeScript SDK supplies its helpers
from `@quality-sh/provenance/rules`.

The scanner matches by shape, not by what the name resolves to: a call to anything named
`rule` with a quoted string id can bind a primary implementation, even when it is not the
identity helper. Ids that match no Rule then show up as warnings under
`--validate-rules`.

The universal floor in every language remains the comment channel:
`@provenance rule: <rule-id>` immediately above the function. Add
`@provenance verification: <method>` for a verification site. Comments scan alongside
native bindings, but are honestly the weaker tier: they can drift away from the symbol.

## Wiki publication safety

`provenance wiki build` renders the complete corpus into a sibling staging
directory and installs that directory only after every page, asset, and
ownership marker has been written. The default `.provenance/wiki` directory is
generator-owned. An explicit `--out` is adopted only when it is absent, empty,
or contains the recognized `.provenance-wiki-output.json` marker. A recognized
marker grants Provenance ownership of the entire directory; do not put
caller-owned files in a marked output.

Publication refuses symlink and non-directory output roots, unsafe lock paths,
unknown marker versions, and unexplained stage, backup, or lock artifacts. It
does not treat a transaction-looking filename, parseable journal, or nonce as
proof of ownership. These failures leave the inspected output and ambiguous
artifacts untouched and report the paths requiring operator attention.

For ordinary errors before installation, the old output is unchanged. If the
old directory has already moved aside and installation returns an error, the
publisher restores it before returning the typed error. A rollback failure is
reported explicitly and leaves all evidence in place rather than guessing.
Once the completed stage is installed, cleanup failures are reported as
`ok_with_cleanup_required` warnings, not as a rolled-back failure, because
recursive backup cleanup may already be partial.

This is an ordinary-error transaction, not a claim of crash atomicity. Process
termination or power loss can leave the fixed sibling lock, stage, or backup;
the next build fails closed and never automatically deletes or restores those
paths. An operator must inspect them and explicitly choose which generation to
keep. Renames are same-filesystem because staging is a sibling, but durability
still depends on the filesystem and storage honoring successful writes and
renames.

## JSONL merge driver

Canonical state is one record per line, so git's line merge invents conflicts
where two branches touched different records. `provenance merge-jsonl` merges by
record id instead. The repository `.gitattributes` already routes state files to
it:

```
.provenance/state/**/*.jsonl merge=provenance-jsonl
```

Git does not carry driver commands in the repository, so every clone runs this
once:

```sh
git config merge.provenance-jsonl.name "Provenance canonical JSONL merge"
git config merge.provenance-jsonl.driver "provenance merge-jsonl %O %A %B --output %A --path %P"
```

Until a clone does, git silently falls back to its usual line merge for those
files. `provenance` must be on the `PATH` git runs with; otherwise use an
absolute path to the binary.

The four placeholders are the command's whole contract. `%O %A %B` are the
positional base, ours, and theirs temporary files. `--output %A` writes the
merged records back over the ours file, which is what git reads. `--path %P` is
the repository path the result belongs at: git hands the driver temporary
files, so this is the only way the merge learns which record type the file
holds. Run by hand, both flags are optional; `--output` alone also serves as the
path when no `--path` is given.

The command exits non-zero when the merge conflicts or when the merged records
would not survive a direct write, and git then leaves the path unmerged for a
human. Merging is a write, so the merged records face the write-time checks:
every shard that holds a record kind with declared relations (sources,
requirements, resolutions, rules, topics, questions, boundaries) is
deserialized as its record type, and a record missing a required relation
(a rule or resolution with no requirement) fails the merge naming that
record, rather than storing it for `provenance check` to find later.
Requirement and Rule merge results are also compared with the merge ancestor,
and a new or statement-changed record with an ASD-STE100 Issue 9 Rule 8.1
finding is rejected before the result path is written. Other per-scope
families merge without typed validation today.

The JSON report names each conflicting record, its kind (`add_add`,
`divergent_edit`, or `delete_modify`), and the base, ours, and theirs
pre-images.

## Immutable graph references

Use the commit-then-issue handoff when another system needs an immutable graph input:

```sh
# 1. Finish and validate canonical graph changes.
provenance check --repo . --format json
git add .provenance/state
git commit -m "Update provenance graph"

# 2. Issue a reference. Omitted --commit means HEAD and requires the selected
#    scope's relevant canonical state to match both the index and working tree.
provenance graph-reference issue --repo . --scope default > graph-reference.json

# 3. Attach graph-reference.json to any external work item. Correlation is
#    generic and does not change deterministic reference identity or trigger
#    proposal surfacing.
provenance graph-reference issue --repo . --scope default \
  --correlation-system github --correlation-key owner/repo#42

# 4. Consumers verify and read only the pinned Git revision.
provenance graph-reference show --repo . --reference graph-reference.json
provenance graph-reference verify --repo . --reference graph-reference.json
provenance graph-reference exact-export --repo . --reference graph-reference.json
```

Pass `--commit <revision>` to `issue` to pin an explicit commit; names and abbreviated
IDs are resolved to a full commit immediately. Explicit pins and all read operations
ignore working-tree graph changes. Implicit `HEAD` permits unrelated source, cache, and
other-scope changes but rejects selected-scope graph changes until they are committed.

All four operations emit versioned JSON. Reference identity is idempotently derived
from the Git repository roots, `.provenance/state`, scope, full commit ID, and canonical
graph digest. Exact exports include only graph-bearing sources, domains, requirements,
boundaries, topics, questions, resolutions, and rules; they do not add proposal,
promotion, collaboration, or workflow-specific fields. The
exact-export document carries the same `graph_digest` as the reference it was cut
from, so it verifies offline, with no repository in hand:
`provenance validate graph-reference-export --input export.json` recomputes the
digest over the graph that travelled and refuses a document whose recorded digest
is not that hash. Failures are typed as `missing`, `mismatched`, or `incomplete`
in their error text.

Inspect the closed JSON Schema contracts with:

```sh
provenance schema show graph-reference --format json
provenance schema show graph-reference-export --format json
```

Skill distribution commands embed the top-level `skills/*/SKILL.md` product skills in the
binary: `provenance skills list --format json`,
`provenance skills show provenance-fork-tournament`, and
`provenance skills install [--global] [--copy] [--force] --format json`. Local installs
write canonical skill files to `.agents/skills/` and link them into `.claude/skills/`;
`--copy` writes Claude skill directories instead of symlinks. `provenance prime` reports
whether the canonical skills are installed and prints the repo-root install command;
shaping/ideation commands emit a non-blocking stderr hint when skills are missing,
suppressible with `--quiet`.

Ideation JSON flags accept inline JSON or `@path/to/payload.json`. Artifact helpers:
`provenance schema show contribution|synthesis-packet|proposal|assertion|disposition --format json` prints
canonical record schemas, and `provenance validate contribution|synthesis-packet|proposal|assertion|disposition
--input artifact.json --format json` validates full closed records, including nested stable IDs,
unknown-field rejection, and assertion evidence cardinality.
Contributions and synthesis packets support intentional `--replace`. Proposal definitions,
assertions, and dispositions are immutable; divergent duplicate IDs fail closed.
Swarm backtrace runs can land durable run outputs with
`provenance swarm-backtrace land --scope <scope> --run-dir <run-dir> --format json`.
Every proposal whose synthesis has exact ownership, positive supporting claims, and no
contested or blocking gate must include an immutable assertion. Missing or invalid evidence
rejects the whole batch, and swarm merge output cannot contain dispositions. Qualification uses
the complete existing plus incoming aggregate, so neither a proposal-only follow-up nor a later
synthesis-only follow-up can bypass assertion requirements.

Create a proposal with optional assertion lineage using repeatable `--builds-on
<assertion-id>`. After synthesis has exact `proposal_id` ownership, positive owned evidence,
and no contested claim or blocking gate, record `provenance proposals assert --id <id>
--proposal-id <proposal> --synthesis-packet-id <packet> --supporting-claim-id <claim>`.
Only then may `dispositions create` record accepted, rejected, or deferred state.
The actor ID must appear in the manifest allowlist configured by repeatable
`provenance init --disposition-actor-id`; this is local audit attestation, not cryptographic
authentication. Re-running `init` preserves manifest settings whose flags are omitted; use
`--clear-disposition-actors` to empty the allowlist explicitly.

Demand-driven proposal review uses `provenance proposals surface`. Pass one or more exact,
repository-relative `--changed-path` values to surface undisposed proposals whose own
evidence sites are touched. Pass `--target-type <type> --target-id <id>` when current work
already names an explicit proposal territory; both target flags are required together and
may be combined with changed paths. Results include every matching proposal and the
`evidence_site` or `territory` reasons it surfaced, in deterministic order, and each nested
proposal carries its derived `proposed` or `asserted` state. `topics claim` returns the
claimed topic plus proposals targeting that topic, its anchor requirement, or its explicit
artifact links. Consultation and claim persistence share one publication operation: a
lifecycle read failure writes no claim. The surface itself is a read-time view and writes
no queue or trigger state.

An accepted `dispositions create` record may link a human's action to the canonical
source, requirement, resolution, or rule it produced using `--canonical-artifact-type` and
`--canonical-artifact-id`. Use that existing link for ratification-through-action; commits
and external issue IDs are not canonical artifact types. Every direct create, import, check,
and materialization resolves the exact `(scope, artifact type, artifact ID)` and fails without
persisting when it is missing, belongs to another scope, or exists only under another type.

Optionally correlate the immutable disposition with an external action using all four of
`--external-system`, `--external-scope`, `--external-kind`, and `--external-key`. The exact
four-part tuple supports issues, commits, tickets, deployments, and other actions without
adding workflow-specific fields. Equal keys in another system, external scope, or action kind
are distinct. Correlation is audit metadata: it does not replace disposition or proposal
identity, and a duplicate disposition cannot mutate it.
Dispositions do not rewrite proposal definitions; `proposals list` derives effective state.

Relation commands, one per reference field: `requirements refines set|clear --requirement-id <id> [--target-id <requirement-id>]`, `requirements depends-on add|clear`, `requirements supersedes add|clear`, `requirements spawned-by set|clear`, `requirements source-ref add|clear --requirement-id <id> --source-id <id>`, `rules requirement add|clear --rule-id <id> --target-id <requirement-id>`, `rules resolution add|clear`, `resolutions requirement add|clear --resolution-id <id> --target-id <requirement-id>`, `resolutions supersedes add|clear`, `sources supersedes add|clear --source-id <id> --target-id <source-id>`, and `questions contradicts set|clear --id <question-id> [--target-id <requirement-id>]`. `add` and `set` refuse a missing target and a `refines`, `depends_on`, or `supersedes` cycle; `clear` refuses the last entry of a required list ("a rule needs one requirement", "a resolution needs one requirement"). `rules create` and `resolutions create` take `--requirement-id` (required, repeatable); `rules create --resolution-id`, `requirements create --refines|--spawned-by|--depends-on|--supersedes`, and `questions create --contradicts` set the same fields at creation.

Rule read commands: `rules list` gives one line per rule in the scope, carrying id, status,
severity, a cut statement, and the source document and section it cites; `rules show --id
<rule-id>` prints one rule whole. `traceability <rule-id>` walks the chain behind a rule and
returns only the relations it crossed, not the whole scope.

Shaping turn-state commands: `questions create` requires `--method` (grill, prototype, research, verify, or task); `topics claim/release/close` and `questions claim/release/answer` manage claim state (claiming an already-claimed item fails and reports the holder; closing a topic or answering a question clears its claim); `requirements fog set/show/clear` manages the deliberately unstructured fog text on an anchor requirement.

Creation commands accept enriched v1 metadata for cloud-imported projects. Examples: `sources create --source-type legislation --reference "Department guidance" --commit-pin 5e1f2a9c4b6d8e0f1234567890abcdef12345678 --effective-date 1714521600000 --review-date 1717200000000 --supersedes source_2024`, `requirements create --status discovery --description "Research note" --domain-id domain_policy`, `resolutions create --status draft --confidence 0.9 --context "Code scan" --input-type regulatory --input-reference "Program manual" --input-summary "Reviewed rules" --made-by "Analyst" --approved-by "Approver" --approved-at 1714780800000 --supersedes res_2024`, `rules create --status draft --source-document docs/policy.md --source-section "Expiry limits"` (these fields are citations, not implementation bindings), and `proposals create --confidence 0.83`. Confidence values must be between `0.0` and `1.0`; source commit pins must be 7-64 hexadecimal characters.
