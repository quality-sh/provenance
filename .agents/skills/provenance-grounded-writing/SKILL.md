---
name: provenance-grounded-writing
description: Write specific, evidence-grounded statements and behaviour descriptions for requirements, rules, sources, resolutions, and boundaries — not generic capability language or meeting minutes. Use before calling `requirements create`, `rules create`, `sources create`, `resolutions create`, or `boundaries create`, especially for a root or mid-level requirement, a statement merging several candidates, or a resolution's position and rationale.
---
<!-- Installed by provenance 0.2.3; content hash fnv1a64:3db8da7ffaf75d29 -->

# Grounded writing

Atomic Rule statements read sharp. Requirements above them read like a
capability list — "provides identity, reporting, comms, integrations" — vague enough to fit
any product. The cause is usually altitude, not wording.

## Does this need a requirement?

Ask whether the answer adds a broad obligation. A module's existence is a code fact,
not a Requirement. A fact can answer a Question without a new record. A precise atomic
obligation belongs in a Rule. A choice among alternatives belongs in a Resolution.
Do not create all three record types for each answer.

Attach the record to the lowest Requirement whose obligation it concerns. Read the
Question's Topic, its Requirement, and the children before selecting that Requirement.
The session anchor is not a default parent for every decision.

A spawned Requirement must add meaning beyond the Resolution position. Use `refines`
for its parent and `spawned_by` only for its producing Resolution. A direct Requirement
needs no Resolution producer. Identical statements require a meaning check, not a
mandatory rewrite; different words can still repeat the same obligation.

## Does this need a rule, or is it still a requirement?

A Rule is an identified atomic behavioural obligation that refines a Requirement. A
Resolution may produce it, and it may exist before implementation or verification. So the
question is: **is this a precise, testable behavioural obligation rather than a broad
contract or capability?**

- The obligation is atomic and testable: write the Rule. Bind and verify it when those
  relationships exist.
- The statement still combines several obligations or only names a capability: keep
  sharpening the Requirement instead of minting a vague Rule.

Rules follow behavioural obligations, not code shape. Prose intent lives in the
Requirement, the Rule, and any Resolution that produced it. A Rule without a primary
implementation is Unimplemented—an ordinary state that `coverage scan --validate-rules`
reports rather than hiding.

## Four tests for a drafted statement

- **Swap.** Drop in an unrelated company or product name. Still reads true? It's a capability
  label, not a decision — name the actual mechanism or threshold instead.
- **Name.** Does it name a mechanism, threshold, actor, or error condition — or a category
  ("identity," "reporting," "comms")? Categories aren't requirements.
- **Evidence.** Is there a real locator for the claim — a source clause or a place in the
  code — or is it explicitly provisional? For graph grounding, add a citation with
  `requirements <id> update --relationships-json <json> --if-match <etag>`; otherwise gap
  reporting emits `missing_source_refs`. `fog`
  is unstructured text attached to a Requirement, while `unsupported` / `exploratory`
  mark ideation evidence or speculation. The CLI reports grounding gaps; it does not
  prevent an ungrounded Requirement or Rule from being active. A Rule's
  `--source-document` and `--source-section` cite source material; they do not count as
  an implementation binding. Implementation bindings come from scanner-recognized
  attributes, helpers, decorators, or comments.
- **Climb.** Summarizing several children? Name the one decision they share, not a noun list
  joined by "and." No shared decision — narrow the parent, don't fake a summary.

## Statement tight, behaviour in the description

The statement is one decision in one plain clause. If you're chaining clauses with "and" to
fit the source, the destination, the mechanism, and the retry policy into a single sentence,
you've overpacked it — a climb-test failure in a long coat. Name the decision in the
statement and put the behaviour in `--description`.

A description says how the product behaves, in the present tense: its states, what moves it
between them, what the person sees, and one example. Write it for a builder who
has not read the conversation. It does not retell the brief, does not say who confirmed
what, and does not list what remains to be shaped. History goes on the record's thread;
unknowns go in fog or a Question. One closing `Open:` line may name them. For other record
types, use their context fields, such as a resolution's `--rationale` or `--context`.

Write for whoever reads it next — another agent, a developer, or a non-technical
stakeholder signing off. Plain beats verbose. Say what's true and stop.

Overpacked: "Approval limits shall sync from the Finance Policy service into the local cache
within atomic transactions and refresh via the job queue." Better — statement: "Approval
limits are sourced from the Finance Policy service, not authored locally."

Description as minutes — bad: "The finance lead confirmed in the shaping call that limits
come from the policy service. The original brief mentions a cache. Refresh cadence and
failure behaviour remain to be shaped." Description as spec — good: "Approval limits come
from the Finance Policy service. States: current, stale, unavailable. A refresh runs every
hour; a failed refresh keeps the last limits and marks them stale. A stale limit still
approves and shows its age to the approver. An unavailable limit blocks approval and shows
'Limits unavailable, try again'. Example: a limit raised at 09:10 applies by 10:10. Open:
how old a stale limit may be before it blocks approval."

## Good vs. bad

Fictional product, "ExpenseFlow."

Source — bad: name "Finance policy", reference "internal doc" — no way to find the clause.
Good:

```sh
provenance sources create --scope <scope> \
  --id source_finance_policy_v3 \
  --name "Finance Policy v3 (2026 revision), section 4.2" \
  --source-type policy \
  --reference "confluence:FIN-204#delegated-authority" \
  --format json
```

Requirement, root — bad: "ExpenseFlow shall provide platform horizontals (identity, expense
capture, approvals, reporting, audit, integrations)." Good: "Comply with Finance Policy v3's
delegated-authority approval chain."

Requirement, mid — bad: "ExpenseFlow shall enable end-to-end expense operations." Good:
"Expenses that exceed an employee's delegated authority limit require second-approver
sign-off before entering the payment run."

Rule — bad: "The system shall enforce approval validation" — restates a classification
and names no atomic, testable behaviour.

Rule — good: "An expense strictly above the submitting employee's delegated authority
limit needs a second approver." The statement holds no semicolon, because the write gate
applies ASD-STE100 Issue 9. Put the behaviour at the limit in a second Rule, or in the
description. `--source-document` and `--source-section` cite where the obligation was
read, which here is the policy clause:

```sh
provenance rules create --scope <scope> \
  --id rule_exp_apr_003 \
  --requirement-id <requirement_id> \
  --statement "An expense strictly above the submitting employee's delegated authority limit needs a second approver" \
  --severity high \
  --source-document "Finance Policy v3 (2026 revision)" \
  --source-section "4.2 Delegated authority" \
  --format json
```

Do not put the file where you expect to write the code into `--source-document`. These
fields are citations to source material. They name no home for the code and reserve no
file for it. A Rule holds no planned code location at all; the location becomes known
when a binding exists.

Bind the implementation in the code, so the scanner can see the relationship, and bind
the test that runs the threshold cases with the same id and its method—in Rust,
`#[rule("rule_exp_apr_003")]` on `requires_second_approver` and
`#[verifies("rule_exp_apr_003", examples)]` on the test. If no primary implementation
exists—the limit is re-checked inline in three controllers, or nobody has written it
yet—the Rule remains valid and is Unimplemented. The `provenance-shaping` skill has the
full Rule and binding flow, and the audit rules that go with it.

Resolution — bad: position "Require a second approver above a threshold," rationale
"Finance Policy section 4.2 says so." The citation does not explain a choice.
Good: context "A high limit permits large expenses without a second check. A variable
limit cannot be delivered before the payment run." Position "Set the approval limit
at $2,000." Rationale "The $2,000 limit reduces unchecked expenses compared with $5,000.
A fixed limit can be delivered before the payment run; variable limits cannot."
Put the policy quote in a citation or input. Do not create a child Requirement that
only repeats the selected limit.

Direct Requirement: "Owners can set the mascot appearance." No alternatives were
compared, so no Resolution is needed. Direct Rule: "A share link expires after seven
days." Its Requirement can ground it without a Resolution.

Choice with no spawned record: select a robot instead of an animal because a repair
state must not suggest injury. The Resolution contains that problem and comparison.
Choice with a separate obligation: select configurable expiry instead of a fixed
period because owners need different access periods. A child Requirement adds the
obligation that owners receive advance notice of link expiry; `refines` identifies link access as its
parent and `spawned_by` identifies the producing Resolution.

Read review acceptance for the current content revision separately from lifecycle.
Answer text and `made_by` are not evidence of acceptance. Omit guessed authorship.
An active Rule with a proposed Resolution can have independent grounding in a
Requirement or Source. Do not report that lifecycle pair alone as unsupported.
