#!/usr/bin/env bash

validate_version() {
  local version=${1-}
  local semver='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-([0-9A-Za-z-]+)(\.([0-9A-Za-z-]+))*)?(\+([0-9A-Za-z-]+)(\.([0-9A-Za-z-]+))*)?$'
  if [[ ! "$version" =~ $semver ]]; then
    printf 'release version must be an exact SemVer without a leading v: %s\n' "$version" >&2
    return 2
  fi

  local without_build=${version%%+*}
  if [[ "$without_build" == *-* ]]; then
    local prerelease=${without_build#*-}
    local identifier
    local identifiers
    IFS=. read -r -a identifiers <<< "$prerelease"
    for identifier in "${identifiers[@]}"; do
      if [[ "$identifier" =~ ^[0-9]+$ && "$identifier" == 0* && "$identifier" != 0 ]]; then
        printf 'numeric prerelease identifiers must not have leading zeros: %s\n' \
          "$version" >&2
        return 2
      fi
    done
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
  local expected_name=$3
  local binary=$4
  local output

  if ! output=$("$binary" --version 2>&1); then
    channel_failure "$channel" "$binary --version failed"
    return 1
  fi
  printf '%s\n' "$output"
  local expected_output="$expected_name $expected_version"
  if [[ "$output" != "$expected_output" ]]; then
    channel_failure "$channel" \
      "$binary reported '$output'; expected '$expected_output'"
    return 1
  fi
}

assert_initialized_repository() {
  local channel=$1
  local repository=$2
  local path

  for path in \
    .provenance/state/manifest.json \
    .gitignore \
    AGENTS.md
  do
    if [[ ! -f "$repository/$path" ]]; then
      channel_failure "$channel" "initializer did not create $path"
      return 1
    fi
  done
  if ! grep -Fqx '.provenance/cache/' "$repository/.gitignore"; then
    channel_failure "$channel" "initializer did not add the cache path to .gitignore"
    return 1
  fi
  if ! grep -Fqx '## Provenance' "$repository/AGENTS.md"; then
    channel_failure "$channel" "initializer did not add the Provenance section to AGENTS.md"
    return 1
  fi

  local skill
  for skill in \
    provenance-fork-tournament \
    provenance-grounded-writing \
    provenance-shaping \
    provenance-swarm-backtrace
  do
    for path in \
      ".agents/skills/$skill/SKILL.md" \
      ".claude/skills/$skill/SKILL.md"
    do
      if [[ ! -f "$repository/$path" ]]; then
        channel_failure "$channel" "initializer did not install $path"
        return 1
      fi
    done
  done

}

assert_provenance_check() {
  local channel=$1
  local repository=$2
  local provenance_binary=$3
  if ! "$provenance_binary" check --repo "$repository"; then
    channel_failure "$channel" "provenance check failed for the initialized repository"
    return 1
  fi
}

capture_initialized_repository() {
  local channel=$1
  local repository=$2
  local destination=$3
  shift 3
  local paths=(
    .provenance/state
    .gitignore
    .agents/skills
    .claude/skills
    AGENTS.md
    "$@"
  )
  local path

  if [[ -e "$destination" ]]; then
    channel_failure "$channel" "snapshot destination already exists: $destination"
    return 1
  fi
  for path in "${paths[@]}"; do
    if [[ ! -e "$repository/$path" && ! -L "$repository/$path" ]]; then
      channel_failure "$channel" "cannot snapshot missing initialized path: $path"
      return 1
    fi
  done

  mkdir -p "$destination"
  if ! tar -C "$repository" -cf - -- "${paths[@]}" |
    tar -C "$destination" -xf -
  then
    channel_failure "$channel" "could not capture initialized repository bytes"
    return 1
  fi
}

assert_initialized_snapshots_equal() {
  local channel=$1
  local first=$2
  local second=$3
  if ! git diff --no-index --no-ext-diff --exit-code -- "$first" "$second"; then
    channel_failure "$channel" "the second initialization changed repository bytes"
    return 1
  fi
}
