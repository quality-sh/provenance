#!/usr/bin/env bash
# Throwaway: time the stale query stages and count git processes.
set -euo pipefail
BIN=$(realpath "$1"); SRC=$(realpath "$2"); WORK=$3
rm -rf "$WORK"; mkdir -p "$WORK"; R="$WORK/repo"
git clone --quiet --no-hardlinks "$SRC" "$R"
cd "$R"
BASE=$(git rev-parse HEAD~1)
echo "base $BASE head $(git rev-parse HEAD)"
echo "diff files: $(git diff --name-only $BASE HEAD | wc -l)"
"$BIN" materialize --format json > /dev/null 2>&1 || true
echo "== CLI timings (5 runs)"
for i in 1 2 3 4 5; do
  /usr/bin/time -f "cli %e s" "$BIN" api /rules --format json --method GET --query query=stale --query base=$BASE --query head=HEAD > "$WORK/cli.json"
done
echo "== CLI answer digest: $(sha256sum < "$WORK/cli.json")"
head -c 600 "$WORK/cli.json"; echo
echo "== process count for one CLI call"
strace -f -qq -e trace=execve -o "$WORK/strace.txt" "$BIN" api /rules --format json --method GET --query query=stale --query base=$BASE --query head=HEAD > /dev/null
grep -c 'execve(' "$WORK/strace.txt" || true
grep -o 'execve("[^"]*", \[[^]]*' "$WORK/strace.txt" | sed 's/execve("[^"]*", //' | cut -c1-120 | sort | uniq -c | sort -rn | head -40
echo "== HTTP"
PROVENANCE_STALE_PROFILE=1 "$BIN" review --repo "$R" --repository-id bench --scope default --no-open > "$WORK/review.out" 2> "$WORK/review.err" &
PID=$!
for i in $(seq 1 100); do [ -s "$WORK/review.out" ] && break; sleep 0.2; done
LINE=$(head -1 "$WORK/review.out")
EP=$(echo "$LINE" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d["endpoint"])')
TOK=$(echo "$LINE" | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d["bearer"])')
for i in $(seq 1 12); do
  : > "$WORK/review.err"
  curl -s -o "$WORK/http.json" -w "http %{time_total}s status %{http_code}\n" -H "Authorization: Bearer $TOK" "${EP%/}/rules?query=stale&base=$BASE&head=HEAD"
done
echo "== stage profile of last HTTP call"
cat "$WORK/review.err"
echo "== git processes for one HTTP call"
: > "$WORK/review.err"
sudo strace -f -qq -p $PID -e trace=execve -o "$WORK/strace2.txt" & SP=$!
sleep 1
curl -s -o /dev/null -w "http traced %{time_total}s\n" -H "Authorization: Bearer $TOK" "${EP%/}/rules?query=stale&base=$BASE&head=HEAD"
sleep 1; sudo kill -INT $SP || true; sleep 0.5
sudo chmod a+r "$WORK/strace2.txt"; grep -c 'execve(' "$WORK/strace2.txt" || true
grep -o 'execve("[^"]*", \[[^]]*' "$WORK/strace2.txt" | sed 's/execve("[^"]*", //' | cut -c1-120 | sort | uniq -c | sort -rn | head -40
kill $PID || true
