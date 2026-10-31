# Shaping recovery evaluation

This harness runs real CLI sessions in fresh repositories. It retains each owner prompt, runner event stream, graph snapshot, model identity, binary hash, and commit. It does not replace the blind judge or owner approval.

First run the protocol probe:

```sh
node tools/shaping-recovery-eval/run.mjs --probe --agents codex,claude
```

The probe uses a real Question read, then resumes the saved session in another process. The adapters use the installed runner protocols. Codex uses `exec --json` and `exec resume`; Claude uses print stream JSON and `--resume`. See the [official Codex non-interactive documentation](https://learn.chatgpt.com/docs/non-interactive-mode). The probe records the exact runner and model names. Set `SHAPING_CODEX_MODEL` or `SHAPING_CLAUDE_MODEL` before both sides of a comparison to use another fixed model. Do not change it between paired trials.

Run the scorer checks through a real CLI artifact:

```sh
SHAPING_EVAL_CLI=/absolute/path/to/provenance node --test tools/shaping-recovery-eval/score.test.mjs
```

Run three baseline trials for each host:

```sh
node tools/shaping-recovery-eval/run.mjs --agents codex,claude --trials 3 \
  --base d981e0bd10f00900772bfc49544caa43d17f3154 --candidate HEAD
```

The runner obtains artifacts through the remote Cargo shim by full path. Optional `--base-cli` and `--candidate-cli` paths use already built artifacts; retain the build run that proves each path's commit. `--output` sets an evidence directory outside source control. Each trial starts with the same graph, policy, and owner answers. Use `--resume` with the same output directory after a controller fault; the runner checks the saved owner prompts and runner identity before it continues. Completed turns are not replayed. Use `--packets-only` to rebuild blind packets from retained raw event streams without running either host again. The base stores prerequisite meaning in Question text; the candidate also stores the explicit IDs. Both use the same independent dependency oracle. A new session reads the saved handoff midway through each trial. A real competing claim is released through the CLI at the fixed owner turn. Claude print sessions support `/compact`; the probe records its manual compaction event and checks recovery after the stored Topic claim and Question method change. This is a separate case. Codex exec does not expose forced compaction in the probed protocol; this is a separate limit.

The default stage stops after the baseline. Publish only `blind-packets/` and its rubric as a secret Gist through Git. Keep `packet-mapping.json`, build records, and raw transcripts outside that Gist. Wait for blind scores and owner approval of reminder and departure thresholds. Then run the candidate with the same hosts, models, scenario, and three trial numbers:

```sh
node tools/shaping-recovery-eval/run.mjs --agents codex,claude --trials 3 \
  --base d981e0bd10f00900772bfc49544caa43d17f3154 --candidate HEAD \
  --stage candidate --approved-thresholds /absolute/path/to/approval.json
```

The approval document holds an owner approval reference and integer `reminders` and `method_departures` thresholds. Candidate results still need blind scoring. The structural scorer flags prerequisite transitions and unanswered accepted Questions. It cannot prove the wording of an accepted decision or the order of two writes in one shell command. The blind judge checks accepted facts from graph state, not promises in the dialogue, and resolves order from the event evidence. Release requires zero missing accepted decisions and zero prerequisite violations in every trial, plus both approved thresholds. An unavailable host, missing transcript, missing handoff, unknown order, or pending score leaves the evaluation incomplete.

## Review view and spec tree cases

The fixture anchor `req_pay` refines `req_payroll`, and the repository has the
reviewer `owner`. Each trial has its own user cache and a `BROWSER` stub. The
stub records each page that the CLI opens, with its `root` and `focus`. The
last turn records the review state of every record and the impact view and
spawned Requirements of every Resolution. Codex runs with loopback network
access and the trial cache as a writable root, so it can start the review host.
Codex exec ends the processes of a turn when the turn ends; the next turn
starts the host again.

The scorer reads only the anonymous packets:

```sh
node tools/shaping-recovery-eval/score.mjs --packets /absolute/path/to/blind-packets
```

A packet passes when every case has no findings:

| Case | Finding |
|---|---|
| `view_before_first_question` | No page opened in the first turn, no page link in the first reply, or a command stopped the host. |
| `anchor_inside_parent_tree` | No first-turn page with the parent as root, or any page with the anchor as root. |
| `tree_branches_by_decision` | A new Rule that is not under a Requirement below the anchor, a Rule that does not name the Resolution that spawned its Requirement, an accepted Requirement outside the anchor's tree, or a choice that no Resolution spawned. |
| `resolution_shows_downstream` | A new Resolution with no review entry, no spawned Requirement, or an empty impact view. |
| `every_record_submitted` | A created Requirement, Rule, Resolution, Boundary, or Source in the final graph with no review submission. |
| `prerequisite_order` | A Question claimed or answered while a prerequisite from the independent list is open at the end of the turn. |
| `accepted_decisions` | An accepted Requirement ID is missing or does not state its fact. |
| `refused_decisions` | A Requirement, Rule, or Resolution position adopts a refused idea. |
| `fresh_reads` | A work turn writes before an unfiltered, read-only read of the Topic and each changed Question. |

The `fresh_reads` case reads each CLI call in a shell command separately: each pipeline stage that runs the binary or a shell variable such as `$P`. A help option makes only its own call a help request. Text in quotes, heredoc bodies and comments is not a call.

The structural cases do not replace the blind check of method departures and
reminders. `--guidance-commit` installs the skill and design document from a
commit, so a run before and a run after a guidance change use the same CLI.
`--turns` limits a smoke run, and `--parallel` runs all trials at once.
`--approved-thresholds` takes inline JSON that cites the owner approval.

Evidence stays outside the repository. Cite exact retained Gist revisions from Sources and Resolution inputs. Keep status and approval references on provenance-lziv. Never publish the packet mapping with the blind packets.

The candidate fixture replaces the installed shaping skill and design document
with the current repository files. The CLI artifact stays fixed. Record the
artifact commit separately from the guidance commit.

For the separately supported compaction counterpart, run:

```sh
node tools/shaping-recovery-eval/run.mjs --probe --agents claude --candidate-guidance \
  --cli /absolute/path/to/provenance --output /absolute/path/to/compaction
```

After all six candidate trials and the counterpart finish, mix the retained
baseline packets with the candidate packets:

```sh
node tools/shaping-recovery-eval/run.mjs --mix-with /absolute/path/to/baseline \
  --candidate-packets /absolute/path/to/candidate \
  --compaction-evidence /absolute/path/to/compaction/probe-claude/compaction.json \
  --output /absolute/path/to/mixed
```

The mixed set has twelve conversation packets and two separate compaction
packets. Each packet has a new random ID. Identity fields and local paths are
removed. The mapping stays outside the packet directory. Stop for blind scoring;
do not use the structural scorer to judge these sessions.

A guidance revision after the blind review reruns only the candidate trials and
the compaction counterpart. Omit `--mix-with` to package these seven packets
without a baseline:

```sh
node tools/shaping-recovery-eval/run.mjs \
  --candidate-packets /absolute/path/to/candidate \
  --compaction-evidence /absolute/path/to/compaction/probe-claude/compaction.json \
  --output /absolute/path/to/rerun-blind
```
