---
name: provenance-grounded-writing
description: Write specific, evidence-grounded statements for requirements, rules, sources, resolutions, and boundaries — not generic capability language. Use before calling `requirements create`, `rules create`, `sources create`, `resolutions create`, or `boundaries create`, especially for a root or mid-level requirement, a statement merging several candidates, or a resolution's position and rationale.
---

# Grounded writing

Atomic Rule statements read sharp. Requirements above them read like a
capability list — "provides identity, reporting, comms, integrations" — vague enough to fit
any product. The cause is usually altitude, not wording.

## Does this need a requirement?

Ask: did someone make a decision here, or does this just name a module that exists anyway?

- A module existing is a fact about the code: "The system has a billing module." Not a
  requirement.
- A choice is a decision: "Expenses over the delegated authority limit require
  second-approver sign-off." Someone picked a threshold and a control — that's a
  requirement.

If it's the former, don't create it. Put the fact on the node the real decision lives under,
or leave it as fog.

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
  code — or is it explicitly provisional? For graph grounding, attach a requirement source with
  `requirements source-ref add` (which also creates a `references` edge), or create a valid
  `references` edge directly; otherwise gap reporting emits `missing_source_refs`. `fog`
  is unstructured text attached to a Requirement, while `unsupported` / `exploratory`
  mark ideation evidence or speculation. The CLI reports grounding gaps; it does not
  prevent an ungrounded Requirement or Rule from being active. A Rule's
  `--source-document` and `--source-section` cite source material; they do not count as
  an implementation binding. Implementation bindings come from scanner-recognized
  attributes, helpers, decorators, or comments.
- **Climb.** Summarizing several children? Name the one decision they share, not a noun list
  joined by "and." No shared decision — narrow the parent, don't fake a summary.

## Statement tight, context in the description

The statement is one decision in one plain clause. If you're chaining clauses with "and" to
fit the source, the destination, the mechanism, and the retry policy into a single sentence,
you've overpacked it — a climb-test failure in a long coat. Name the decision in the
statement; for requirements and rules, move the elaboration to `--description`, where being
fuller is fine. For other record types, use their available context fields, such as a
resolution's `--rationale` or `--context`.

Write for whoever reads it next — another agent, a developer, or a non-technical
stakeholder signing off. Plain beats verbose. Say what's true and stop; a statement that
reads at a glance beats an exhaustive one.

Overpacked: "Approval limits shall sync from the Finance Policy service into the local cache
within atomic transactions and refresh via the job queue." Better — statement: "Approval
limits are sourced from the Finance Policy service, not authored locally." description: the
sync path, its transactionality, and the refresh mechanism.

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
limit needs a second approver; an expense at the limit does not." In this codebase,
`ApprovalService.php`'s `requires_second_approver($expense, $employee)` is its primary
implementation. Its code location may also serve as a source citation on the record,
but that citation does not bind the implementation:

```sh
provenance rules create --scope <scope> \
  --id rule_exp_apr_003 \
  --requirement-id <requirement_id> \
  --statement "An expense strictly above the submitting employee's delegated authority limit needs a second approver; an expense at the limit does not" \
  --severity high \
  --source-document ApprovalService.php \
  --source-section requires_second_approver \
  --format json
```

Then bind the implementation in the code so the scanner can see the relationship, and
bind the test that runs the threshold cases with the same id and its method—in Rust,
`#[rule("rule_exp_apr_003")]` and `#[verifies("rule_exp_apr_003", examples)]`. If no
primary implementation exists—the limit is re-checked inline in three controllers—the
Rule remains valid but Unimplemented. The `provenance-shaping` skill has the full Rule
and binding flow.

Resolution — bad: position "Require a second approver above a threshold," rationale "Team
decided this was the right approach" — no number, no alternative, no reason one lost. Good:
position "Set the delegated authority limit at $2,000 per employee tier; require a second
approver above that," rationale "Chosen over a flat $5,000 limit (too permissive) and
per-tier variable limits (unshippable before Q3 close); $2,000 matches the median threshold
across a 12-peer benchmark."
