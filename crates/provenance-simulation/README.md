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
suite of `provenance-store` on the simulated storage. `tests/refused_post.rs`
runs fixed cases of a message post whose publish is refused, in the engine
job, as a probe of the engine layer.

The probes are fixed cases next to the random test of each layer. A probe
that breaks a law fails with "simulation probe broke law <law> in layer
<layer>". A probe writes no failure file.

L3 follows each Requirement that a write creates or updates (its existence
and its description) and the Topic (its status and its claim). The model
starts from the initial state: the first Requirement has no description,
the Requirements that a create makes do not exist, and the Topic is open
with no claim. Each write gives its record a value from the write itself,
not from its answer, so the value is known also when the answer is not:

| Write | Value of its record when it lands |
| --- | --- |
| Create | The Requirement exists with no description |
| Update | The Requirement has the description of the update |
| Claim | The Topic is open and the actor of the claim holds it |
| Release | The Topic is open with no claim |
| Archive of the first Requirement | The Topic is archived with no claim |

The product refuses a claim of a Topic that is not open, and a release
unless the actor holds the claim; an archive clears the claim. So a claim
or a release that lands shows an open Topic.

A certain write (its last publish landed, and it did not stop or fail)
gives its record its value. A write that is not certain (a stop, an unknown
outcome, or a failure after a landed publish) can have landed or not, so
its record holds its value or a value that the record could hold before
the write. A write that landed no publish changes no record. In the CLI
layer, the writes that run at once are one group, a stopped write and a
write merged from a branch are not certain, and a failed write that did not
stop changes no record. The engine law and the CLI law use the same model
(`src/expected.rs`).

A write on a moved base: in the engine and contract layers, the engine
reads, a write outside lands, and the engine publishes on the moved base.
The CLI layer cannot stop a process between its read and its publish. For
an update, it reads the etag, merges the outside write, and sends the
update with that etag. For another write, it merges the outside write and
then runs the write, so that write does not meet a moved base.

The job summary gives the counts of the random cases up to the first broken
law. After a broken law, Hegel shrinks the case; each run of the shrink adds
only to "shrink runs".

A stop of a CLI write kills the write process at a point of its publish
that the test sees on disk: an even `at` when the transaction directory of
the write appears, an odd `at` when the publication marker appears while
that transaction exists. The order of a publish is in
`provenance_store::storage::local::whole_set::land`. After the kill, the
transaction, the marker, the phase of the marker and the live state tell
where the stop landed. Each stop that a case draws adds 1 to one of these
rows:

| Row | What the stop did |
| --- | ----------------- |
| stops before the marker | It killed the write while its transaction existed and no marker was there. |
| stops after the marker and before the install | It killed the write after the marker and before the install of the new state. |
| stops after the install and before the answer | It killed the write after the install of the new state. |
| stops after the answer | The write gave its answer, a success or a refusal, before the kill. |
| stops of a write that ended before its stop point | The write ended before the test saw its point, and the state did not change. |
| stops whose point the test did not see | The write ended and changed the state, but the test did not see its point. |
| stops of a write that the simulation did not send | The CLI layer has no route for the write, or the read of its etag refused. |
| stops with no waiting write of the engine | No write of the engine waited, so the stop had no write. |
| stops of a write that did not end in its limit | The write did not end in the process limit; the case breaks L0. |

The row "stops that left no live state" counts the stops that killed a
write before its answer and left no live state. The next read recovers
the live state from the transaction.

Three probes of the CLI layer stop a write on each run. One stops a claim at
its transaction, and the stop must land before the marker. One stops a
claim at the marker, and the stop must land after the marker and before the
install; a read after the stop must remove the marker and give the state
before the claim. One stops each kind of write that the simulation sends
at its transaction, and each stop must kill its write before its answer.

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

An issue "a planted fault was not found" opens when a planted test fails
with "simulation check broken". It names each such test with its replay
command.

An issue "test <test> failed" opens for each other test that failed, for
example a check of a probe that is not a law, a panic outside a law, or a
failed planted test. An issue "test <test> did not end in its time limit"
opens for each test that nextest stopped. Each gives the failure text and
the replay command.

An issue "failed without a broken law" means that the job did not succeed
and the log does not show the end of the tests, or that no test gave a
report. The build failed, or the job stopped at its time limit. Read the
log of the run.

Each issue of a layer opens apart from the other issues of the layer, so
one failure does not hide another one in the same night.

## Replay a case

The replay command of an issue has this form:

```sh
HEGEL_SEED=<seed> cargo nextest run --profile simulation --run-ignored all \
  -p provenance-simulation --test engine -E 'test(=random_cases_keep_the_laws)'
```

The report script takes the package, the test target and the test from
the status line that nextest prints for the failed test, for example
`FAIL [ 6.102s] (7/7) provenance-simulation::contract
random_cases_keep_the_laws_through_the_contract`. The random tests are
ignored tests, so the command needs `--run-ignored all`. The command sets
the variables of the job: `HEGEL_SEED`, `HEGEL_TEST_CASES` when the run
sets it, and `SIMULATION_L6` in the CLI layer. The CLI layer checks L6
after each write; the macOS and Windows jobs check it only at the end of
each case, so the replay command of their failures sets
`SIMULATION_L6=end`.

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
- The CLI jobs can fail on L6 when the state holds two withdrawals of one
  review submission: bead provenance-chgs.
