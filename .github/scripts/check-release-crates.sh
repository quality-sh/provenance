#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
failed=0
while IFS= read -r crate_name; do
  status=$(curl --silent --show-error --location --retry 3 \
    --connect-timeout 15 --max-time 60 --output /dev/null --write-out '%{http_code}' \
    --user-agent provenance-release-preflight \
    "https://crates.io/api/v1/crates/$crate_name") || status=000
  if [[ "$status" != 200 ]]; then
    if [[ "$status" == 404 ]]; then
      echo "::error::$crate_name is missing: publish a placeholder with an API token, then add the trusted publisher before the tag." >&2
    else
      echo "::error::Cannot confirm $crate_name on crates.io (HTTP $status); stop the release." >&2
    fi
    failed=1
  fi
done < "$script_directory/../release-crates.txt"
exit "$failed"
