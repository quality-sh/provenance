#!/usr/bin/env bash
# Scans the commits that the git log options select, with the gitleaks binary
# in $GITLEAKS, in the repository in the current directory. Gitleaks always
# reads ./.gitleaksignore, so a scanned tree could hide its own findings. This
# script removes that file and gives gitleaks only the copy on origin/main.
# Usage: gitleaks-scan.sh <git log options> [gitleaks option ...]
set -euo pipefail
log_opts=$1
shift
ignore_dir=$(mktemp -d)
trap 'rm -rf -- "$ignore_dir"' EXIT
if git cat-file -e refs/remotes/origin/main:.gitleaksignore 2>/dev/null; then
  git show refs/remotes/origin/main:.gitleaksignore > "$ignore_dir/gitleaksignore"
else
  : > "$ignore_dir/gitleaksignore"
fi
rm -rf -- .gitleaksignore
echo "gitleaks log options: $log_opts"
"$GITLEAKS" git . --no-banner --redact --exit-code 1 \
  --gitleaks-ignore-path "$ignore_dir/gitleaksignore" --log-opts="$log_opts" "$@"
