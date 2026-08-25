#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 <crate-name> <version> <crate-archive>" >&2
  exit 2
fi

crate_name=$1
version=$2
archive=$3

if [[ ! "$crate_name" =~ ^[a-z0-9_-]+$ ]]; then
  echo "invalid crate name: $crate_name" >&2
  exit 2
fi
if [[ ! "$version" =~ ^[0-9A-Za-z.+-]+$ ]]; then
  echo "invalid crate version: $version" >&2
  exit 2
fi
if [[ ! -f "$archive" ]]; then
  echo "crate archive does not exist: $archive" >&2
  exit 2
fi

registry_response=$(mktemp)
trap 'rm -f "$registry_response"' EXIT

registry_status() {
  env -u CARGO_REGISTRY_TOKEN curl \
    --silent \
    --show-error \
    --output "$registry_response" \
    --write-out '%{http_code}' \
    --header "User-Agent: provenance-release/$version (+https://github.com/quality-sh/provenance)" \
    "https://crates.io/api/v1/crates/$crate_name/$version"
}

verify_registry_checksum() {
  yanked=$(jq -r \
    'if (.version | has("yanked")) then .version.yanked else "missing" end' \
    "$registry_response")
  if [[ "$yanked" != false ]]; then
    echo "crates.io reports $crate_name@$version as yanked or malformed" >&2
    exit 1
  fi
  registry_checksum=$(jq -er '.version.checksum' "$registry_response")
  local_checksum=$(sha256sum "$archive" | awk '{print $1}')
  if [[ "$registry_checksum" != "$local_checksum" ]]; then
    echo "crates.io checksum for $crate_name@$version does not match $archive" >&2
    exit 1
  fi
}

status=$(registry_status)
case "$status" in
  200)
    verify_registry_checksum
    echo "$crate_name@$version is already on crates.io; skipping"
    exit 0
    ;;
  404)
    ;;
  *)
    echo "crates.io returned HTTP $status for $crate_name@$version; refusing to publish" >&2
    exit 1
    ;;
esac

# Rehearse without a credential. The upload then uses the exact clean checkout
# without rebuilding it while the credential is in the child environment.
env -u CARGO_REGISTRY_TOKEN \
  cargo publish --registry crates-io --dry-run --locked --package "$crate_name"

if cargo publish --registry crates-io --no-verify --locked --package "$crate_name"; then
  echo "published $crate_name@$version"
else
  publish_status=$?
  status=$(registry_status)
  if [[ "$status" != 200 ]]; then
    echo "cargo publish failed and crates.io does not show $crate_name@$version" >&2
    exit "$publish_status"
  fi
  verify_registry_checksum
  echo "cargo publish failed after crates.io accepted $crate_name@$version; continuing"
fi

# A dependent package cannot publish until Cargo can read this exact version
# from the registry index.
attempt=1
max_attempts=${CRATES_IO_VISIBILITY_ATTEMPTS:-60}
while (( attempt <= max_attempts )); do
  if env -u CARGO_REGISTRY_TOKEN \
    cargo info --registry crates-io "$crate_name@$version" >/dev/null 2>&1; then
    exit 0
  fi
  sleep 10
  ((attempt += 1))
done

echo "Cargo cannot read $crate_name@$version after publication" >&2
exit 1
