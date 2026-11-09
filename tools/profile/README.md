# API profile

The `API profile` workflow (`.github/workflows/api-profile.yml`) records
where each local API call spends its CPU time. It is not a required check.

It runs:

- on the push to main that a pull request merge makes, when the merge changes
  Rust code, a `Cargo.toml`, `Cargo.lock`, `.cargo/`, this directory,
  `tools/api-latency/` or the workflow file. It compares the merged commit
  with the main baselines before it, and it is the newest baseline after it.
- with workflow_dispatch on any branch, from the Actions page or with
  `gh workflow run api-profile.yml --ref <branch>`. It compares the branch
  with the newest main baselines. When the branch has an open pull request,
  the run posts or updates the profile comment of that pull request.

It does not run on a pull request or on a push to a pull request branch. A
reviewer starts it for a pull request branch. A new dispatch on a branch
cancels the older run on that branch.

## Rows and time limit

A run takes at most 15 minutes from start to end, setup and build included,
also when the Rust cache is cold. Each job has `timeout-minutes: 15`. The
full benchmark with 2 write repeats and an opt-level 3 build took 17 minutes,
so a run has three cuts. With them, a run took 11 minutes (build 6, the
slowest profile job 4, report 1).

- The build uses opt-level 1, not the opt-level 3 of a release. A cold build
  at opt-level 3 or 2 took 8 to 12 minutes, by runner CPU, and left too little
  time for the profile; at opt-level 1 it took 6 to 8 minutes. Most of the
  build is the workspace crates (`provenance-store`, `provenance-transport`,
  `provenance-mcp`), which build one after the other, so more codegen units
  or no LTO did not help. The shares are of an opt-level 1 binary: less
  inlining than in a release, so small functions (iterators, serde helpers)
  have their own frames and a larger share. Runs and baselines use the same
  build, so a change between them is still a change of the code. To see the
  shares of a release build, run the API latency benchmark locally under perf.
- The catalog writes (each write operation and the read after it) run through
  MCP only (`--route-writes MCP`). HTTP, MCP and `provenance api` all call the
  same write handlers, and the writes were most of the time of each way. MCP
  had the shortest write rows. The HTTP and CLI ways keep all catalog reads.
- Repeats: 3 warm repeats of each read and 1 of each write after the cold
  call (the `reads` and `writes` inputs of a dispatch). Most of the time of a
  write row is the setup of each write, so a write repeat costs more than a
  read repeat.

The rows of a run:

- HTTP: the start of the host, the first read on a new clone, and each
  catalog read.
- MCP: the same, each catalog write and the read after it, and each action
  of each MCP tool.
- CLI: the first read on a new clone, each catalog read through
  `provenance api`, each porcelain command and each action of
  `provenance <id> <action>`.

## How it works

1. The `Build` job builds the release CLI with frame pointers
   (`RUSTFLAGS=-C force-frame-pointers=yes`) at opt-level 1. Product code
   does not change.
   At the same time, the `Inferno` job installs a pinned inferno release
   (the flame graph tools) with `cargo install`
   (`.github/actions/setup-inferno`) and gives it to the later jobs as an
   artifact.
2. Three `Profile` jobs, one for each way in (HTTP, MCP and CLI), run in
   parallel. Each runs the API latency benchmark (`tools/api-latency/run.mjs`)
   for its way in, the rows of the section above in the benchmark sequence,
   under one `perf record` at 1999 Hz. The sequence is necessary: a row that
   runs alone on a new clone sees less state and gives different times.
3. The harness writes the process id and the start and end time of each timed
   call to `windows-<way>.json`. `label.mjs` gives each CPU sample of that
   process in that window to the row. Samples of setup calls, of node, of git
   and of a host between calls are not counted. When one function is on the
   stack two or more times in a row, as the poll functions of async code are,
   the stacks keep it once. This changes no share.
4. `inferno-collapse-perf` folds the stacks. The `Report` job runs
   `analyze.mjs`, which writes the tables and the comparison, and
   `inferno-flamegraph` draws one SVG for each row. On a dispatch, the same
   job posts or updates the pull request comment.

A row is one way in and one call, for example `HTTP GET /rules/{id}` or
`CLI provenance check --format json`. The labels are the same as in the API
latency report. A write row also has a row for the read after it
(`..., then the first read`).

## Where to find each output

A dispatch run makes two artifacts: `api-profile` (small: the tables and
`profile.json`) and `api-profile-stacks` (large: the stacks and the flame
graphs). A main push names them `api-profile-baseline` and
`api-profile-baseline-stacks`.

| What | Where | Download |
|---|---|---|
| Sticky comment: the rows with the largest share changes, and links | The pull request conversation, after a dispatch on its branch; the comment starts with `<!-- api-profile -->` | No |
| Job summary: the same, and a table of all rows | The run page, and the log of the step `Publish the summary` of the `Report` job | No |
| `profile.json`: the data of the tables and the comparison, for a program | `api-profile` | Yes |
| `functions.json`: for each row, the inclusive counts of each function with 0.5 percent or more, in total and in each half; a later run compares with it | `api-profile` | Yes |
| `summary.md`, `comment.md` | `api-profile` | Yes |
| `rows/<file>.md`: top functions and the change from the baseline for one row | `api-profile` | Yes |
| `run/results-<way>.json`: the call times under perf, for reference only | `api-profile` | Yes |
| `run/windows-<way>.json`, `run/samples-<way>.json`, `run/runner-<way>.json`: the call windows, the sample counts and the runner CPU | `api-profile` | Yes |
| `rows/<file>.folded`: the collapsed stacks of one row | `api-profile-stacks` | Yes |
| `rows/<file>.svg`: the flame graph of one row, for a person | `api-profile-stacks` | Yes |

The `file` of a row is in `profile.json` and in the last column of the rows
table of the job summary. It is the row label in lower case with a short hash,
so it is the same in each run.

`profile.json` has `commit`, `run`, `frequency`, `samples` (the sample
counts), `limits` (the noise limits), `runners` (the CPU of each part),
`baselines` (the commit, run and CPUs of each, newest first), `newRows`,
`goneRows` and `rows`. Each row has `way`, `call`, `label`,
`file`, `calls`, `wallMs` (the sum of the call times under perf), `samples`,
`cpuMsPerCall`, `top.inclusive` and `top.self`
(the top 25 functions with `share` and `samples`), and `baseline`: null, or
`runs` (the number of baselines with this row), `samples` (the sample count
of each), `dispersion` (null with one baseline), `grew` and `shrank` (the 10
largest changes, each with `base`, `now`, `change`, `z` and `overNoise`) and
`overNoise` (the number of functions with a change over the noise).

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
- The baseline share of a function is the mean of its share in the newest
  three main baselines (fewer when fewer exist).
- A change is "over the noise" when it is at least 5 points and at least 4
  standard errors (`z`). The standard error is the larger of two values:
  - The error of a count test on the samples, times the square root of the
    row dispersion. The dispersion tells how much more the shares of the row
    change between the baseline runs than the count test expects. It is pooled
    over all functions of the row with a mean share from 2 to 98 percent, so
    two or three baselines give a usable value. Shares change between runs
    even for the same code: the runner speed changes how much background work,
    for example a projection catch-up, falls inside a call.
  - The error from the halves of the calls. Each run puts the first, third,
    ... call of a row in half `a` and the second, fourth, ... call in half
    `b`; the difference between the halves shows how much a share changes
    from call to call.

  With one baseline the dispersion is not known, and the comment says so. With
  2 to 4 calls in a row these errors are estimates, so use `z` as a guide, not
  as a proof. Four runs of the same code, each compared with the other three,
  gave changes over the noise in 4 to 49 of 744 rows, most in the rows that
  read after a write (those runs had 2 write repeats and all CLI rows). A runner with a different CPU model moves more shares;
  the header names the CPU of the run and of each baseline.
- Kernel functions have the suffix `_[k]` (from `inferno-collapse-perf
  --kernel`). They are in the tables and the flame graphs, but not in the
  comparison: the memory work of the kernel (page faults, page allocation,
  cgroup charges) changes with the runner host, not with the code. Their
  samples still count in the inclusive share of the program functions that
  call them. A row with fewer than 200 samples has a noisy profile; its
  `rows/<file>.md` tells so. When main changed between the baselines, the
  dispersion of a changed row is larger, so the comparison is more careful.
- `CPU ms for each call` is the sample count divided by the rate and by the
  number of timed calls. It is an estimate under perf; use the API latency
  workflow for times.
- The baselines are the `api-profile-baseline` artifacts of main push runs;
  artifacts of other branches are not used. The comment and the summary name
  the commit and the run of each. A dispatch profiles the branch head, not a
  merge with main: when main changed after the branch point, part of a change
  can come from main.
- For the counts, function names have no Rust hash suffix and no C library
  symbol version. A function with one version for each set of CPU features
  has one name: `__memmove_avx_unaligned_erms` and
  `__memmove_avx512_unaligned_erms` are both `__memmove_simd_unaligned`.
  Runners have different CPUs, so without this a change of runner looks like
  a change of code. The stacks and the flame graphs keep the real names.
  `[unknown]` frames are code with no frame pointer, for example parts of the
  standard library.

## Files

- `profile.mjs`: the sample labels, the share counts, the dispersion and the
  comparison. No file input or output.
- `label.mjs`: reads `perf script` output and writes the samples of each row
  with the row file name as the process name.
- `analyze.mjs`: writes `profile.json`, the per-row files and the Markdown.
- `profile.test.mjs`: unit tests. Run them with
  `node --test tools/profile/*.test.mjs`.
