# Durable record text evaluation

This evaluation compares writing guidance. It adds no statement finding or write
refusal. The fixture and process harness come from PR #422. This comparison uses
five fresh sessions for each runner in each arm. Use only the two approved CLI
runners. Keep all trial output outside Git.

Configure `runners` as documented in PR #422. Each entry has `kind`, `executable`,
`args` and an explicit `model`. The default turn limit is ten minutes. Claude uses
stream output, empty setting sources and resumed sessions. A timeout stops the
process group. A failed or incomplete turn stops the comparison.

```sh
node --test tools/shaping-eval/test.mjs tools/shaping-eval/integration.test.mjs
node tools/shaping-eval/run.mjs --config /tmp/runners.json --variant base --output /tmp/probe --probe true
node tools/shaping-eval/run.mjs --config /tmp/runners.json --variant base --output /tmp/base
node tools/shaping-eval/run.mjs --config /tmp/runners.json --variant candidate --output /tmp/candidate
```

The baseline pins guidance to `d981e0bd10f00900772bfc49544caa43d17f3154`.
The fixture is `fixtures/task.json`. Its response indices are source locators.
The harness supplies the fixture in each trial repository as `task.json`.
Keep inputs, CLI, settings and model identities equal across arms. Confirm actual
identity from session metadata. `PROVENANCE_BIN` selects the CLI binary.

Public list and review commands capture graph records after each turn. This is
the evidence route used by PR #422. Public export refuses review-bearing graphs.
The final canonical copy is retained for audit only. The harness does not write
canonical graph files. Trial prompts, outputs, skills, hashes, graph views,
seed records and authored records remain in the external output directory.

## Human scoring

Count each newly authored or changed record with statement, description,
position, rationale or context once. Exclude unchanged seed records. Count a
record as contaminated if any of these fields repeats its author, lifecycle,
acceptance or review state. Product actors are not record authors. Phrase
matches identify candidates only. A person must check all records, including
paraphrases. Text must remain true after acceptance or rejection.

Grade each rationale for four dimensions: correct position, correct source
locator, actual reason, and rejection of the supplied alternative. Use the
response index as the locator in this fixture. Where no alternative is supplied,
mark the dimension true only if the rationale does not invent a rejected option.

Mix record packets from both arms. Remove paths, timestamps, runner names,
model names and arm information before review. Retain the mapping separately.
Labels contain `id`, boolean `contaminated`, and, for rationales, a `grounding`
object with boolean `position`, `locator`, `reason` and `alternative` fields.
Missing labels cannot pass. Retain reviewed labels and their evidence locators.

```sh
node tools/shaping-eval/score.mjs /tmp/authored.json /tmp/labels.json /tmp/counts.json
```

Sum the counts for each runner and arm. `compare` requires a strictly lower
contamination rate for each runner and no decrease in any grounding rate.
A zero baseline cannot prove improvement. Extend the same frozen task set for
both arms before candidates, or stop for revised approval. Failed sessions do
not count. Retain each attempt. Human scoring is required before sign-off.
Tests are implementation aids and have no Rule bindings.
