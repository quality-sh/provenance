#!/usr/bin/env bash
# Run the API latency harness once per group in three passes:
#   time   - plain, the same repeats as the benchmark
#   strace - strace -f over the harness (git processes, fsync, file calls)
#   perf   - perf record, cpu-clock samples and every context switch, with call chains
set -euo pipefail
BIN=$(realpath "$1"); OUT=$(realpath -m "$2"); WORK=$3; PASSES=${4:-time strace perf}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
export PROF_REAL=$BIN
while IFS='|' read -r name filter ways reads writes; do
  case "$name" in ''|'#'*) continue;; esac
  for pass in $PASSES; do
    dir="$OUT/$pass/$name"; mkdir -p "$dir"
    export PROF_LOG="$dir/calls.tsv"; : > "$PROF_LOG"
    r=$reads; w=$writes
    if [ "$pass" = time ]; then r=20; w=10; fi
    harness=(node tools/api-latency/run.mjs --bin "$HERE/wrap.sh" --source . --work "$WORK/$pass-$name"
      --out "$dir/results.json" --reads "$r" --writes "$w" --ways "$ways" --filter "$filter")
    echo "::group::$pass $name"
    start=$(date +%s)
    case "$pass" in
      time) "${harness[@]}" 2> "$dir/harness.log" || echo "harness failed: $pass $name" ;;
      strace) strace -f -qq -tt -T -yy -s 400 -o "$dir/strace.txt" \
          -e trace=%process,%file,fsync,fdatasync,syncfs,sync_file_range,copy_file_range,pread64,pwrite64,flock,fcntl \
          "${harness[@]}" 2> "$dir/harness.log" || echo "harness failed: $pass $name"
        gzip -f "$dir/strace.txt" ;;
      perf) perf record -q -k mono -g -e cpu-clock/freq=1999/ -e context-switches/period=1/ \
          -o "$dir/perf.data" -- "${harness[@]}" 2> "$dir/harness.log" || echo "harness failed: $pass $name" ;;
    esac
    echo "$pass $name: $(( $(date +%s) - start )) s"; tail -3 "$dir/harness.log"
    echo "::endgroup::"
  done
done < "$HERE/groups.txt"
