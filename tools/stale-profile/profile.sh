#!/usr/bin/env bash
# Throwaway: the stages of one stale query through the CLI after the change.
set -euo pipefail
BIN=$(realpath "$1"); SRC=$(realpath "$2"); WORK=$3
rm -rf "$WORK"; mkdir -p "$WORK"; R="$WORK/repo"
git clone --quiet --no-hardlinks "$SRC" "$R"
cd "$R"
BASE=$(git rev-parse HEAD~1)
echo "base $BASE head $(git rev-parse HEAD)"
echo "diff files: $(git diff --name-only "$BASE" HEAD | wc -l)"
"$BIN" materialize --format json > /dev/null 2>&1 || true
stale() { "$BIN" api /rules --format json --method GET --query query=stale --query "base=$BASE" --query head=HEAD; }
echo "== process start: provenance --version (10 runs)"
for i in $(seq 1 10); do /usr/bin/time -f "version %e s" "$BIN" --version > /dev/null; done
echo "== CLI wall time (10 runs)"
for i in $(seq 1 10); do /usr/bin/time -f "cli %e s" bash -c "$(declare -f stale); BIN=$BIN BASE=$BASE stale" > "$WORK/cli.json"; done
echo "== CLI answer digest: $(sha256sum < "$WORK/cli.json")"
for i in 1 2 3 4 5 6; do
  echo "== CLI stage profile, run $i"
  PROVENANCE_STALE_PROFILE=1 stale 2>&1 > /dev/null | grep PROF
done
echo "== process count for one CLI call"
sudo apt-get install -y strace > /dev/null 2>&1 || true
strace -f -qq -e trace=execve -o "$WORK/strace.txt" "$BIN" api /rules --format json --method GET --query query=stale --query "base=$BASE" --query head=HEAD > /dev/null
grep -c 'execve(' "$WORK/strace.txt" || true
grep -o 'execve("[^"]*", \[[^]]*' "$WORK/strace.txt" | sed 's/execve("[^"]*", //' | cut -c1-100 | sort | uniq -c | sort -rn | head -20
echo "== HTTP stage profile (cold, then warm)"
PROVENANCE_STALE_PROFILE=1 "$BIN" review --repo "$R" --repository-id bench --scope default --no-open > "$WORK/review.out" 2> "$WORK/review.err" &
PID=$!
for i in $(seq 1 100); do [ -s "$WORK/review.out" ] && break; sleep 0.2; done
LINE=$(head -1 "$WORK/review.out")
EP=$(echo "$LINE" | python3 -c 'import json,sys;print(json.load(sys.stdin)["endpoint"])')
TOK=$(echo "$LINE" | python3 -c 'import json,sys;print(json.load(sys.stdin)["bearer"])')
for i in $(seq 1 6); do
  : > "$WORK/review.err"
  curl -s -o "$WORK/http.json" -w "http %{time_total}s status %{http_code}\n" -H "Authorization: Bearer $TOK" "${EP%/}/rules?query=stale&base=$BASE&head=HEAD"
  grep -a PROF "$WORK/review.err" || true
done
kill $PID || true
