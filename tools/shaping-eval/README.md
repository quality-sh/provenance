# Shaping recovery evaluation

This harness runs real CLI sessions in fresh repositories. It retains each owner prompt, runner event stream, graph snapshot, model identity, binary hash, and commit. It does not replace the blind judge or owner approval.

First run the protocol probe:

```sh
node tools/shaping-eval/run.mjs --probe --agents codex,claude
```

The probe uses a real Question read, then resumes the saved session in another process. The adapters use the installed runner protocols. Codex uses `exec --json` and `exec resume`; Claude uses print stream JSON and `--resume`. See the [official Codex non-interactive documentation](https://learn.chatgpt.com/docs/non-interactive-mode). The probe records the exact runner and model names. Set `SHAPING_CODEX_MODEL` or `SHAPING_CLAUDE_MODEL` before both sides of a comparison to use another fixed model. Do not change it between paired trials.

Run the scorer checks through a real CLI artifact:

```sh
SHAPING_EVAL_CLI=/absolute/path/to/provenance node --test tools/shaping-eval/score.test.mjs
```

Run three baseline trials for each host:

```sh
node tools/shaping-eval/run.mjs --agents codex,claude --trials 3 \
  --base d981e0bd10f00900772bfc49544caa43d17f3154 --candidate HEAD
```

The runner obtains artifacts through the remote Cargo shim by full path. Optional `--base-cli` and `--candidate-cli` paths use already built artifacts; retain the build run that proves each path's commit. `--output` sets an evidence directory outside source control. Each trial starts with the same graph, policy, and owner answers. Use `--resume` with the same output directory after a controller fault; the runner checks the saved owner prompts and runner identity before it continues. Completed turns are not replayed. Use `--packets-only` to rebuild blind packets from retained raw event streams without running either host again. The base stores prerequisite meaning in Question text; the candidate also stores the explicit IDs. Both use the same independent dependency oracle. A new session reads the saved handoff midway through each trial. A real competing claim is released through the CLI at the fixed owner turn. Claude print sessions support `/compact`; the probe records its manual compaction event and checks recovery after the stored Topic claim and Question method change. This is a separate case. Codex exec does not expose forced compaction in the probed protocol; this is a separate limit.

The default stage stops after the baseline. Publish only `blind-packets/` and its rubric as a secret Gist through Git. Keep `packet-mapping.json`, build records, and raw transcripts outside that Gist. Wait for blind scores and owner approval of reminder and departure thresholds. Then run the candidate with the same hosts, models, scenario, and three trial numbers:

```sh
node tools/shaping-eval/run.mjs --agents codex,claude --trials 3 \
  --base d981e0bd10f00900772bfc49544caa43d17f3154 --candidate HEAD \
  --stage candidate --approved-thresholds /absolute/path/to/approval.json
```

The approval document holds an owner approval reference and integer `reminders` and `method_departures` thresholds. Candidate results still need blind scoring. The structural scorer flags prerequisite transitions and unanswered accepted Questions. It cannot prove the wording of an accepted decision or the order of two writes in one shell command. The blind judge checks accepted facts from graph state, not promises in the dialogue, and resolves order from the event evidence. Release requires zero missing accepted decisions and zero prerequisite violations in every trial, plus both approved thresholds. An unavailable host, missing transcript, missing handoff, unknown order, or pending score leaves the evaluation incomplete.

Evidence stays outside the repository. Cite exact retained Gist revisions from Sources and Resolution inputs. Keep status and approval references on provenance-lziv. Never publish the packet mapping with the blind packets.
