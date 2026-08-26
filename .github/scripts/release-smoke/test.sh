#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=.github/scripts/release-smoke/lib.sh
source "$script_directory/lib.sh"

fail() {
  printf 'release smoke helper test failed: %s\n' "$*" >&2
  exit 1
}

assert_accepts_version() {
  validate_version "$1" || fail "valid version was rejected: $1"
}

assert_rejects_version() {
  if validate_version "$1" >/dev/null 2>&1; then
    fail "invalid version was accepted: $1"
  fi
}

for version in 0.2.2 1.0.0-rc.1 2.4.6+build.9 1.0.0-alpha.beta; do
  assert_accepts_version "$version"
done
for version in v0.2.2 01.2.3 1.02.3 1.2.03 1.0.0- 1.0.0-alpha..1 1.0.0-01 latest; do
  assert_rejects_version "$version"
done

temporary=$(make_smoke_directory helper-test)
trap 'rm -rf -- "$temporary"' EXIT

fake_binary="$temporary/provenance"
cat > "$fake_binary" <<'SH'
#!/usr/bin/env bash
printf 'not-provenance 0.2.2\n'
SH
chmod 755 "$fake_binary"
if assert_binary_version test 0.2.2 provenance "$fake_binary" >/dev/null 2>&1; then
  fail "binary version check accepted the wrong program name"
fi

cat > "$fake_binary" <<'SH'
#!/usr/bin/env bash
printf 'provenance 0.2.2 extra\n'
SH
if assert_binary_version test 0.2.2 provenance "$fake_binary" >/dev/null 2>&1; then
  fail "binary version check accepted extra output"
fi

cat > "$fake_binary" <<'SH'
#!/usr/bin/env bash
if [[ ${1-} == --version ]]; then
  printf 'provenance 0.2.2\n'
elif [[ ${1-} == check && ${2-} == --repo && -d ${3-} ]]; then
  printf 'check\n' >> "$CHECK_LOG"
  exit 0
else
  exit 2
fi
SH
export CHECK_LOG="$temporary/check.log"
assert_binary_version test 0.2.2 provenance "$fake_binary"

repository="$temporary/repository"
mkdir -p "$repository/.provenance/state" "$repository/.agents/skills" \
  "$repository/.claude/skills"
printf '{}\n' > "$repository/.provenance/state/manifest.json"
printf '.provenance/cache/\n' > "$repository/.gitignore"
printf '# Instructions\n' > "$repository/AGENTS.md"
for skill in \
  provenance-fork-tournament \
  provenance-grounded-writing \
  provenance-shaping \
  provenance-swarm-backtrace
do
  mkdir -p "$repository/.agents/skills/$skill"
  printf '# %s\n' "$skill" > "$repository/.agents/skills/$skill/SKILL.md"
  ln -s "../../.agents/skills/$skill" "$repository/.claude/skills/$skill"
done
printf '[package]\nname = "fixture"\nversion = "0.0.0"\n' > "$repository/Cargo.toml"
printf '# lock\n' > "$repository/Cargo.lock"

if assert_initialized_repository test "$repository" >/dev/null 2>&1; then
  fail "repository assertion accepted AGENTS.md without the Provenance section"
fi
printf '\n## Provenance\nUse the installed workflow.\n' >> "$repository/AGENTS.md"
assert_initialized_repository test "$repository"
if [[ -e "$CHECK_LOG" ]]; then
  fail "repository shape assertion ran provenance check"
fi
assert_provenance_check test "$repository" "$fake_binary"
[[ $(wc -l < "$CHECK_LOG") -eq 1 ]] || fail "provenance check did not run exactly once"
capture_initialized_repository test "$repository" "$temporary/first" Cargo.toml Cargo.lock
capture_initialized_repository test "$repository" "$temporary/second" Cargo.toml Cargo.lock
assert_initialized_snapshots_equal test "$temporary/first" "$temporary/second"

printf '# changed\n' >> "$repository/Cargo.toml"
capture_initialized_repository test "$repository" "$temporary/changed" Cargo.toml Cargo.lock
if assert_initialized_snapshots_equal test "$temporary/first" "$temporary/changed" \
  >/dev/null 2>&1
then
  fail "snapshot comparison accepted changed bytes"
fi

attempts_file="$temporary/attempts"
probe_after_two_failures() {
  local attempts=0
  if [[ -f "$attempts_file" ]]; then
    attempts=$(<"$attempts_file")
  fi
  attempts=$((attempts + 1))
  printf '%s\n' "$attempts" > "$attempts_file"
  ((attempts >= 3))
}
RELEASE_SMOKE_RETRY_ATTEMPTS=3 RELEASE_SMOKE_RETRY_DELAY_SECONDS=0 \
  retry_channel registry-probe probe_after_two_failures
[[ $(<"$attempts_file") -eq 3 ]] || fail "availability probe did not stop after success"

checksum_file="$temporary/SHA256SUMS"
digest=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
printf '%s  archive.tar.gz\n' "$digest" > "$checksum_file"
[[ $(expected_checksum "$checksum_file" archive.tar.gz) == "$digest" ]] ||
  fail "checksum lookup did not return the exact archive digest"
printf '%s  archive.tar.gz\n' "$digest" >> "$checksum_file"
if expected_checksum "$checksum_file" archive.tar.gz >/dev/null 2>&1; then
  fail "checksum lookup accepted duplicate archive entries"
fi

printf 'release smoke helper tests passed\n'
