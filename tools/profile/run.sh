#!/usr/bin/env bash
# One pass of the full API latency harness (HTTP, MCP and CLI, every row), as
# the benchmark runs it, so each row sees the same state as in the benchmark.
#   time   - no tool, the benchmark repeats
#   perf   - perf record for each process of a matched row
#   strace - strace -f for each process of a matched row
set -euo pipefail
BIN=$(realpath "$1"); OUT=$(realpath -m "$2"); WORK=$3; PASS=$4
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
export PROF_REAL=$BIN PROF_LOG=$OUT/calls.tsv PROF_DIR=$OUT
export PROF_MATCH='^(provenance <id> (create|update|submit) |provenance check --format json$|provenance check --graph --format json$|provenance materialize|provenance report build|GET /rules/\{id\}/history|GET /requirements/\{id\}$|POST /requirements$|tool check$|tool change action=(materialize|create|update|submit)( |$))'
: > "$PROF_LOG"
reads=20; writes=10
if [ "$PASS" != time ]; then export PROF_MODE=$PASS; reads=0; fi
start=$(date +%s)
node tools/api-latency/run.mjs --bin "$HERE/wrap.sh" --source . --work "$WORK" \
  --out "$OUT/results.json" --reads "$reads" --writes "$writes" 2> "$OUT/harness.log" || echo "harness failed"
echo "$PASS: $(( $(date +%s) - start )) s"; tail -3 "$OUT/harness.log"
