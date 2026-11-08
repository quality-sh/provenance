#!/usr/bin/env bash
# Give the state of the base commit to the binary of this commit.
#
# Usage: base-state-read.sh <binary> <scratch worktree>
#
# The script runs three checks on the scratch worktree:
#   1. check --graph.
#   2. One list read for each list route in `api --format json`.
#   3. One write: a create of a Topic. The write changes only the scratch
#      worktree. CI never commits the worktree.
#
# Each refusal goes to the job summary. When OVERRIDE is "true", a refusal
# gives a warning, and the script passes. Else a refusal gives an error, and
# the script fails.
set -uo pipefail

binary=$1
scratch=$2
override=${OVERRIDE:-false}
summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}
work=$(mktemp -d)
refused=()
skipped=()
passed=()

note() {
  if [[ "$override" == "true" ]]; then
    echo "::warning::$1"
  else
    echo "::error::$1"
  fi
}

# Record one refusal. The second argument is a file with the output.
refuse() {
  refused+=("$1")
  note "$1"
  if [[ -s "$2" ]]; then
    head -c 2000 "$2"
    echo
    {
      echo "<details><summary>$1</summary>"
      echo
      echo '```text'
      head -c 2000 "$2"
      echo
      echo '```'
      echo "</details>"
    } >> "$work/refusals.md"
  else
    echo "- $1" >> "$work/refusals.md"
  fi
}

# 1. The graph check.
if "$binary" check --graph --repo "$scratch" > "$work/graph.out" 2>&1; then
  passed+=("check --graph")
else
  refuse "check --graph refuses the base state." "$work/graph.out"
fi

# 2. The list reads. The JSON catalog gives all routes, not only the first
# page of the text catalog.
routes=()
if "$binary" api --format json --repo "$scratch" > "$work/api.json" 2> "$work/api.err" \
  && jq -e '.bounds.truncated == false' "$work/api.json" > /dev/null; then
  mapfile -t routes < <(jq -r '.routes[] | select(.method == "get" and (.path | test("^/[a-z-]+$"))) | .path' "$work/api.json")
  if [[ ${#routes[@]} -eq 0 ]]; then
    refuse "The API catalog gives no list routes." "$work/api.json"
  fi
else
  cat "$work/api.err" >> "$work/api.json"
  refuse "The API catalog is not available as a complete JSON list." "$work/api.json"
fi

for route in "${routes[@]}"; do
  out="$work/read.json"
  if "$binary" api "$route" --repo "$scratch" > "$out" 2> "$work/read.err" \
    && jq -e '(has("error") | not) and (.data.items | type == "array")' "$out" > /dev/null 2>&1; then
    count=$(jq '.data.items | length' "$out")
    if [[ "$count" -eq 0 ]]; then
      skipped+=("$route")
      echo "$route: skipped, the base state has no rows"
    else
      passed+=("$route ($count rows on the first page)")
      echo "$route: $count rows on the first page"
    fi
  else
    # An empty output, an output that is not JSON, and an error envelope
    # are all refusals.
    cat "$work/read.err" >> "$out"
    refuse "This binary cannot read $route in the base state." "$out"
  fi
done

# 3. One write. The Topic refines an active Requirement of the base state.
probe="ci_base_state_write_probe"
requirement=$("$binary" api /requirements --repo "$scratch" 2> /dev/null \
  | jq -r '[.data.items[]? | select(.status == "active")][0].id // empty' 2> /dev/null)
if [[ -z "$requirement" ]]; then
  echo "The /requirements read gave no active Requirement for the Topic." > "$work/write.out"
  refuse "The write check did not run." "$work/write.out"
elif "$binary" "$probe" create --type topic --title "Base state write probe" \
    --requirement-id "$requirement" --format json --repo "$scratch" \
    > "$work/write.out" 2> "$work/write.err" \
  && jq -e --arg id "$probe" '(has("error") | not) and .data.id == $id' "$work/write.out" > /dev/null 2>&1 \
  && ! grep -q '"error"' "$work/write.err"; then
  passed+=("create of Topic $probe")
  echo "Write: created Topic $probe under $requirement"
  if [[ -s "$work/write.err" ]]; then
    echo "::warning::The write gave text on stderr: $(head -c 500 "$work/write.err")"
  fi
else
  cat "$work/write.err" >> "$work/write.out"
  refuse "This binary cannot write to the base state." "$work/write.out"
fi

{
  echo "## Base state read"
  echo
  echo "Passed: ${#passed[@]}. Refused: ${#refused[@]}. Skipped: ${#skipped[@]}."
  echo
  for item in "${passed[@]}"; do echo "- Passed: $item"; done
  for item in "${skipped[@]}"; do echo "- Skipped: $item (the base state has no rows)"; done
  if [[ ${#refused[@]} -gt 0 ]]; then
    echo
    echo "### Refusals"
    echo
    cat "$work/refusals.md"
  fi
} >> "$summary"

if [[ ${#refused[@]} -eq 0 ]]; then
  exit 0
fi

if [[ "$override" == "true" ]]; then
  line="The state-format-change label was used. This binary refuses the base state, and this pull request changes .provenance/state. Existing repositories must be initialised again."
  echo "::warning::$line"
  {
    echo
    echo "**$line**"
  } >> "$summary"
  exit 0
fi

exit 1
