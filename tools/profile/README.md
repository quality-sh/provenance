# API profile

The `API profile` workflow (`.github/workflows/api-profile.yml`) records
where each local API call spends its CPU time. It is not a required check.

It runs:

- on a pull request that changes Rust code, a `Cargo.toml`, `Cargo.lock`,
  `.cargo/`, this directory, `tools/api-latency/` or the workflow file;
- on a push to main with the same changes, to make the baseline;
- from the Actions page with workflow_dispatch.

A new push to a pull request cancels the older run of that pull request.

## How it works

1. The `Build` job builds the release CLI with frame pointers
   (`RUSTFLAGS=-C force-frame-pointers=yes`). Product code does not change.
2. Three `Profile` jobs, one for each way in (HTTP, MCP and CLI), run in
   parallel. Each runs the API latency benchmark (`tools/api-latency/run.mjs`)
   for its way in, all rows in the benchmark sequence, under one
   `perf record` at 1999 Hz. They use fewer repeats than the latency
   workflow: 3 for each read and 2 for each write. The full sequence is
   necessary: a row that runs alone on a new clone sees less state and gives
   different times.
3. The harness writes the process id and the start and end time of each timed
   call to `windows-<way>.json`. `label.mjs` gives each CPU sample of that
   process in that window to the row. Samples of setup calls, of node, of git
   and of a host between calls are not counted. When one function is on the
   stack two or more times in a row, as the poll functions of async code are,
   the stacks keep it once. This changes no share.
4. `inferno-collapse-perf` folds the stacks. The `Report` job runs
   `analyze.mjs`, which writes the tables and the comparison, and
   `inferno-flamegraph` draws one SVG for each row. The `Comment` job posts or
   updates the pull request comment. The workflow installs a pinned inferno
   release with `cargo install` (`.github/actions/setup-inferno`).

A row is one way in and one call, for example `HTTP GET /rules/{id}` or
`CLI provenance check --format json`. The labels are the same as in the API
latency report. A write row also has a row for the read after it
(`..., then the first read`).

## Where to find each output

A pull request run makes two artifacts: `api-profile` (small: the tables and
`profile.json`) and `api-profile-stacks` (large: the stacks and the flame
graphs). A main push names them `api-profile-baseline` and
`api-profile-baseline-stacks`.

| What | Where | Download |
|---|---|---|
| Sticky comment: the rows with the largest share changes, and links | The pull request conversation; the comment starts with `<!-- api-profile -->` | No |
| Job summary: the same, and a table of all rows | The run page, and the log of the step `Publish the summary` of the `Report` job | No |
| `profile.json`: the data of the tables and the comparison, for a program | `api-profile` | Yes |
| `summary.md`, `comment.md` | `api-profile` | Yes |
| `rows/<file>.md`: top functions and the change from the baseline for one row | `api-profile` | Yes |
| `run/results-<way>.json`: the call times under perf, for reference only | `api-profile` | Yes |
| `run/windows-<way>.json`, `run/samples-<way>.json`: the call windows and the sample counts | `api-profile` | Yes |
| `rows/<file>.folded`: the collapsed stacks of one row | `api-profile-stacks` | Yes |
| `rows/<file>.svg`: the flame graph of one row, for a person | `api-profile-stacks` | Yes |

The `file` of a row is in `profile.json` and in the last column of the rows
table of the job summary. It is the row label in lower case with a short hash,
so it is the same in each run.

`profile.json` has `commit`, `head`, `run`, `frequency`, `samples` (the sample
counts), `limits` (the noise limits), `baseline` (its commit and run, or
null), `newRows`, `goneRows` and `rows`. Each row has `way`, `call`, `label`,
`file`, `calls`, `wallMs` (the sum of the call times under perf), `samples`,
`cpuMsPerCall`, `top.inclusive` and `top.self`
(the top 25 functions with `share` and `samples`), and `baseline`: null, or
the baseline `samples`, `grew` and `shrank` (the 10 largest changes, each with
`base`, `now`, `change`, `z` and `overNoise`) and `overNoise` (the number of
functions with a change over the noise).

With the gh CLI:

- The comment: `gh pr view <number> --comments`, or
  `gh api repos/quality-sh/provenance/issues/<number>/comments`.
- The runs: `gh run list --workflow api-profile.yml --branch <branch>`.
- The summary in the log: `gh run view <run> --log` and find
  `Publish the summary`.
- The tables: `gh run download <run> -n api-profile`. The stacks and the flame
  graphs: `gh run download <run> -n api-profile-stacks` (about 100 MB).

## How to read it

- A share is the percent of the CPU samples of one row. Inclusive share counts
  a sample for each function on its stack (the function and what it calls);
  self share counts the sample for the function at the top of the stack only.
- The comparison uses inclusive shares, not milliseconds, because the speed of
  the runner changes by up to 30 percent from run to run. A change is in share
  points: `+5.0` means that the function was on 5 percent more of the samples
  of the row.
- A change is "over the noise" when it is at least 2 points and at least 3
  standard errors (`z`, a two-proportion test on the sample counts). A row with
  fewer than 200 samples has a noisy profile; its `rows/<file>.md` tells so.
  The samples of one call are not independent, so use `z` as a guide, not as
  a proof.
- `CPU ms for each call` is the sample count divided by the rate and by the
  number of timed calls. It is an estimate under perf; use the API latency
  workflow for times.
- The baseline is the newest `api-profile-baseline` artifact, which a main
  push makes. The comment and the summary name its commit and its run. On a pull
  request, the profiled commit is the merge of the head into the base; the
  header names the head too. When main changed after the baseline, part of a
  change can come from main.
- Function names have no Rust hash suffix, so one function has the same name
  in each build. `[unknown]` frames are code with no frame pointer, for
  example parts of the standard library.

## Files

- `profile.mjs`: the sample labels, the share counts, the comparison. No file
  input or output.
- `label.mjs`: reads `perf script` output and writes the samples of each row
  with the row file name as the process name.
- `analyze.mjs`: writes `profile.json`, the per-row files and the Markdown.
- `profile.test.mjs`: unit tests. Run them with
  `node --test tools/profile/*.test.mjs`.
