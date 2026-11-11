# provenance-simulation

This crate is the nightly simulation of the write engine. It draws random
sequences of writes and faults, runs them, and checks laws on the result. No
pull request check runs it. The workflow `.github/workflows/simulation.yml`
runs it each night at 02:41 UTC and on a manual start.

## Layers

Each layer is one job, so a failure names one layer.

| Layer | What runs each write |
| --- | --- |
| `engine` | Two or three real engines over one simulated storage. The case decides the order of each storage call. |
| `contract` | The same engines, with each write sent through `provenance_contract::invoke_with`. |
| `cli-linux`, `cli-macos`, `cli-windows` | One `provenance api` process for each write, on a repository on disk (`crates/provenance-cli/tests/simulation_cli.rs`). |

## Laws

| Law | Layers | What it checks |
| --- | --- | --- |
| L0 | all | Each operation or process ends in its time limit, with no panic. |
| L1 | engine, contract | An operation that fails, with no unknown outcome, lands no write. |
| L2 | engine, contract | After an engine stops, the state opens and reads again. |
| L3 | all | A write that succeeded stays unless a later write changed it. |
| L4 | all | Of two writes on one etag, at most one succeeds. |
| L5 | engine, contract | After a moved base, the next write of the section lands. |
| L6 | CLI | The reads after each write equal the reads after a full rebuild. |
| L7 | CLI | Each answer of one writer has the answer kind that the engine gives. |
| L8 | engine, contract | One case gives the same final bytes each time. |
| O1 | contract | No answer is a write failure (500) unless a fault met the operation. |
| O2 | contract | The times of the messages increase in the order that they landed. |
| O3 | contract | A stage lands as one whole set on the revision where it started. |

Each law apart from L0 has a planted test: `tests/engine_planted.rs`,
`tests/contract_planted.rs` and
`crates/provenance-cli/tests/simulation_cli_planted.rs`. A planted test
plants the smallest fault that its law must see. Most faults are in the
simulated storage, the identity or the repository on disk. Three use a
stand-in: the CLI L4 test changes the etag that the run recorded, the CLI L7
test gives the engine a stand-in that refuses a claim, and the O3 test adds
a second stage to a decision. If the law does not see its fault, the test
fails with "simulation check broken", and the issue says that a check is
broken, not a law. `tests/storage_contract.rs` runs the storage contract
suite of `provenance-store` on the simulated storage.

The probes are fixed cases next to the random test of each layer. A probe
that breaks a law fails with "simulation probe <test> broke law <law>" and
the command that runs the probe. It writes no failure file.

A write on a moved base: in the engine and contract layers, the engine
reads, a write outside lands, and the engine publishes on the moved base.
The CLI layer cannot stop a process between its read and its publish. For
an update, it reads the etag, merges the outside write, and sends the
update with that etag. For another write, it merges the outside write and
then runs the write, so that write does not meet a moved base.

The job summary gives the counts of the random cases up to the first broken
law. After a broken law, Hegel shrinks the case; each run of the shrink adds
only to "shrink runs". A stop of a CLI write counts as "during a write" only
when the process runs at the kill and has not printed its answer.

## When a failure issue opens

The report job opens one issue for each layer and law, with the label
`simulation`. A second night with the same failure adds a comment.

1. Read the issue. It gives the law, the detail, the seed, the shrunk steps,
   the replay command and the replay blob.
2. Replay the case on your computer. Use the blob, because it replays on any
   system and any build:
   paste the `#[hegel::reproduce_failure("<blob>")]` line from the issue on
   the random test of the layer, above `fn`, and run the replay command of
   the issue. The seed repeats the run only on the same build.
3. File a bead for the fault, with the issue as its external reference.
4. Fix the engine. Do not change the law to make the case pass. If the law
   itself is wrong, that is a decision for the owner.
5. Close the issue after a night passes with no failure of that law.

A blob of "not printed; the case was not reproduced" means that Hegel ran
the shrunk case again and the case passed. This happens in the CLI layer,
where two processes at once make a case nondeterministic. Use the shrunk
steps of the issue as a fixed case, and run it more than one time.

An issue "probe <test> broke law <law>" gives the replay command of the
probe. The probe is a fixed case, so the command runs the same case on any
system.

An issue "a planted fault was not found" opens each time that a planted
test fails, also on a night with a law issue for the same layer.

An issue "failed without a broken law" means that the job did not succeed,
and no random case, probe or planted test reported a law or a broken check.
The build or a probe test failed, a test panicked outside a law, or the job
stopped at its time limit. Read the log of the run.

## Replay a case

The replay command of an issue has this form:

```sh
HEGEL_SEED=<seed> cargo nextest run --profile simulation --run-ignored all \
  -p provenance-simulation --test engine -E 'test(=random_cases_keep_the_laws)'
```

The random tests are ignored tests, so the command needs `--run-ignored all`.
A run with a case count other than 100 also sets `HEGEL_TEST_CASES`. For the
CLI layer, the package is `provenance-cli` and the test binary is
`simulation_cli`. The CLI layer checks L6 after each write; the macOS and
Windows jobs check it only at the end of each case, so the replay command of
their failures sets `SIMULATION_L6=end`.

## Start a manual run

In the Actions tab, open the workflow "Simulation" and click "Run workflow".
The input `cases` sets the number of cases for each random test (the default
is 100). The input `seed` sets the seed (the default is the run ID). The
input `report` is `issues` or `print`. With `issues`, the report job opens
or comments on issues, as the nightly run does. With `print`, the report job
prints each issue that it would open and each comment that it would add, in
the job log and the job summary, and makes no GitHub write.

## Known failures

Each line names one cause and its tracker. A fix of one cause does not
change the other lines; remove the line of a cause when its fix merges.

- The contract job can fail on O1 when an update of a Requirement that does
  not exist answers a write failure (500): GitHub issue 648.
- The contract job can fail on O1 when a write on a moved base answers a
  write failure (500), for example a message post whose base moved before
  its publish: bead provenance-cf0b. This cause has the same law and layer
  as issue 648, so its failures add comments to the same issue.
- The engine job can fail on L1: bead provenance-4ayr.
- The CLI jobs can fail on L6 when the state holds two withdrawals of one
  review submission: bead provenance-chgs.
