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

fake_npm_bin="$temporary/fake-npm-bin"
mkdir -p "$fake_npm_bin"
cat > "$fake_npm_bin/npm" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$NPM_VIEW_LOG"
SH
chmod 755 "$fake_npm_bin/npm"
export NPM_VIEW_LOG="$temporary/npm-view.log"
PATH="$fake_npm_bin:$PATH" npm_release_packages_are_visible \
  "$script_directory/../../release-targets.json" 0.2.2
expected_npm_views=$(($(jq 'length' "$script_directory/../../release-targets.json") + 2))
[[ $(wc -l < "$NPM_VIEW_LOG") -eq "$expected_npm_views" ]] ||
  fail "npm visibility probe did not check every release package"
for package in @quality-sh/create-provenance @quality-sh/provenance; do
  grep -Fqx "view $package@0.2.2 version" "$NPM_VIEW_LOG" ||
    fail "npm visibility probe omitted $package"
done
while IFS= read -r package; do
  grep -Fqx "view $package@0.2.2 version" "$NPM_VIEW_LOG" ||
    fail "npm visibility probe omitted $package"
done < <(jq -r '.[].npm.name' "$script_directory/../../release-targets.json")

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

cat > "$repository/Cargo.toml" <<'TOML'
[package]
name = "fixture"
version = "0.0.0"

[dependencies]
provenance-sdk = "=0.2.2"
TOML
assert_cargo_sdk_requirement test "$repository/Cargo.toml" 0.2.2
sed -i.bak 's/=0\.2\.2/0.2.2/' "$repository/Cargo.toml"
rm -f -- "$repository/Cargo.toml.bak"
if assert_cargo_sdk_requirement test "$repository/Cargo.toml" 0.2.2 \
  >/dev/null 2>&1
then
  fail "Cargo requirement assertion accepted a non-exact requirement"
fi
sed -i.bak 's/"0\.2\.2"/"=0.2.2"/' "$repository/Cargo.toml"
rm -f -- "$repository/Cargo.toml.bak"

capture_initialized_repository test "$repository" "$temporary/first"
capture_initialized_repository test "$repository" "$temporary/second"
assert_initialized_snapshots_equal test "$temporary/first" "$temporary/second"

printf 'unexpected initializer output\n' > "$repository/unexpected.txt"
capture_initialized_repository test "$repository" "$temporary/unexpected"
if assert_initialized_snapshots_equal test "$temporary/first" "$temporary/unexpected" \
  >/dev/null 2>&1
then
  fail "snapshot comparison ignored an unexpected repository file"
fi
rm -f -- "$repository/unexpected.txt"

printf '# changed\n' >> "$repository/Cargo.toml"
capture_initialized_repository test "$repository" "$temporary/changed"
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

mkdir -p "$temporary/archive-source" "$temporary/archive-output"
printf 'archive fixture\n' > "$temporary/archive-source/file.txt"
python3 - "$temporary" <<'PY'
import sys
import zipfile
from pathlib import Path

root = Path(sys.argv[1])
with zipfile.ZipFile(root / "fixture.zip", "w") as archive:
    archive.write(root / "archive-source/file.txt", "file.txt")
PY
python3 "$script_directory/extract_archive.py" \
  "$temporary/fixture.zip" "$temporary/archive-output"
cmp "$temporary/archive-source/file.txt" "$temporary/archive-output/file.txt"

contract_output="$temporary/release-contract-output"
python3 "$script_directory/../release-contract.test.py"
python3 "$script_directory/../release-contract.py" \
  0.2.2 "$script_directory/../../release-targets.json" > "$contract_output"
jq -e -s '
  (map(select(startswith("version="))) == ["version=0.2.2"]) and
  (map(select(startswith("build-matrix="))) | length == 1) and
  (map(select(startswith("smoke-matrix="))) | length == 1)
' < <(jq -R . "$contract_output") >/dev/null ||
  fail "release contract did not emit one version and both target matrices"

printf 'release smoke helper tests passed\n'
