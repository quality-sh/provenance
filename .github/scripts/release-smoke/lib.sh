#!/usr/bin/env bash

validate_version() {
  local version=${1-}
  if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?(\+[0-9A-Za-z][0-9A-Za-z.-]*)?$ ]]; then
    printf 'release version must be an exact SemVer without a leading v: %s\n' "$version" >&2
    return 2
  fi
}

channel_failure() {
  local channel=$1
  shift
  local message="$*"
  if [[ ${GITHUB_ACTIONS-} == true ]]; then
    printf '::error title=%s distribution::%s\n' "$channel" "$message" >&2
  fi
  printf '%s: %s\n' "$channel" "$message" >&2
}

retry_channel() {
  local channel=$1
  shift
  local attempts=${RELEASE_SMOKE_RETRY_ATTEMPTS:-20}
  local delay=${RELEASE_SMOKE_RETRY_DELAY_SECONDS:-30}

  if [[ ! "$attempts" =~ ^[0-9]+$ ]] || ((attempts < 1 || attempts > 30)); then
    channel_failure "$channel" "RELEASE_SMOKE_RETRY_ATTEMPTS must be from 1 through 30"
    return 2
  fi
  if [[ ! "$delay" =~ ^[0-9]+$ ]] || ((delay > 120)); then
    channel_failure "$channel" "RELEASE_SMOKE_RETRY_DELAY_SECONDS must be from 0 through 120"
    return 2
  fi

  local attempt
  for ((attempt = 1; attempt <= attempts; attempt += 1)); do
    if "$@"; then
      return 0
    fi
    if ((attempt < attempts)); then
      printf '%s attempt %d/%d failed; retrying in %s seconds\n' \
        "$channel" "$attempt" "$attempts" "$delay" >&2
      sleep "$delay"
    fi
  done

  channel_failure "$channel" "failed after $attempts attempts"
  return 1
}

make_smoke_directory() {
  local channel=$1
  local base=${RUNNER_TEMP:-${TMPDIR:-/tmp}}
  if command -v cygpath >/dev/null 2>&1; then
    base=$(cygpath -u "$base")
  fi
  if [[ ! -d "$base" ]]; then
    channel_failure "$channel" "temporary directory does not exist: $base"
    return 1
  fi
  mktemp -d "$base/provenance-${channel}.XXXXXX"
}

expected_checksum() {
  local checksum_file=$1
  local archive=$2
  local found=
  local matches=0
  local digest filename extra

  while read -r digest filename extra; do
    if [[ "$filename" == "$archive" && -z ${extra-} ]]; then
      found=$digest
      matches=$((matches + 1))
    fi
  done < "$checksum_file"

  if ((matches != 1)) || [[ ! "$found" =~ ^[0-9A-Fa-f]{64}$ ]]; then
    channel_failure "GitHub archive" \
      "SHA256SUMS must contain one valid entry for $archive (found $matches)"
    return 1
  fi
  printf '%s\n' "$found" | LC_ALL=C tr '[:upper:]' '[:lower:]'
}

sha256_file() {
  local path=$1
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | { read -r digest _; printf '%s\n' "$digest"; } |
      LC_ALL=C tr '[:upper:]' '[:lower:]'
    return
  fi
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | { read -r digest _; printf '%s\n' "$digest"; } |
      LC_ALL=C tr '[:upper:]' '[:lower:]'
    return
  fi
  channel_failure "GitHub archive" "no SHA-256 program is available"
  return 1
}

assert_binary_version() {
  local channel=$1
  local expected_version=$2
  local binary=$3
  local output

  if ! output=$("$binary" --version 2>&1); then
    channel_failure "$channel" "$binary --version failed"
    return 1
  fi
  printf '%s\n' "$output"
  local reported_version=${output##* }
  if [[ "$reported_version" != "$expected_version" ]]; then
    channel_failure "$channel" \
      "$binary reported a version other than $expected_version: $output"
    return 1
  fi
}
