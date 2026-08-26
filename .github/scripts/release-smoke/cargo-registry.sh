#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=.github/scripts/release-smoke/lib.sh
source "$script_directory/lib.sh"

version=${1-}
validate_version "$version"
channel="crates.io"
smoke_root=$(make_smoke_directory cargo-registry)
trap 'rm -rf -- "$smoke_root"' EXIT

export CARGO_HOME="$smoke_root/cargo-home"
mkdir -p "$CARGO_HOME"
export PATH="$CARGO_HOME/bin:$PATH"

crates_are_visible() {
  local crate
  for crate in \
    provenance-macros \
    provenance-core \
    provenance-scanner \
    provenance-ste100 \
    provenance-store \
    provenance-sdk \
    provenance-cli
  do
    curl --fail --silent --show-error \
      --user-agent "provenance-release-smoke/$version" \
      --output /dev/null "https://crates.io/api/v1/crates/$crate/$version" || return
  done
}

retry_channel "$channel" crates_are_visible
if ! cargo install --locked provenance-cli --version "=$version"; then
  channel_failure "$channel" "provenance-cli $version failed to install after it became visible"
  exit 1
fi

for binary in provenance cargo-provenance; do
  installed_binary="$CARGO_HOME/bin/$binary"
  if [[ ! -x "$installed_binary" ]]; then
    channel_failure "$channel" "provenance-cli $version did not install $binary"
    exit 1
  fi
  assert_binary_version "$channel" "$version" "$binary" "$installed_binary"
done

fixture="$smoke_root/rust-fixture"
mkdir -p "$fixture"
if ! cargo init --lib --name provenance_release_smoke "$fixture"; then
  channel_failure "$channel" "cargo init failed for the Rust fixture"
  exit 1
fi
if ! (cd "$fixture" && cargo provenance init); then
  channel_failure "$channel" "the first cargo provenance init failed"
  exit 1
fi
assert_cargo_sdk_requirement "$channel" "$fixture/Cargo.toml" "$version"
assert_initialized_repository "$channel" "$fixture"
capture_initialized_repository "$channel" "$fixture" "$smoke_root/first-init"

if ! (cd "$fixture" && cargo provenance init); then
  channel_failure "$channel" "the second cargo provenance init failed"
  exit 1
fi
assert_cargo_sdk_requirement "$channel" "$fixture/Cargo.toml" "$version"
assert_initialized_repository "$channel" "$fixture"
capture_initialized_repository "$channel" "$fixture" "$smoke_root/second-init"
assert_initialized_snapshots_equal \
  "$channel" "$smoke_root/first-init" "$smoke_root/second-init"
assert_provenance_check "$channel" "$fixture" "$CARGO_HOME/bin/provenance"

mkdir -p "$fixture/tests"
cat > "$fixture/tests/sdk_macros.rs" <<'RUST'
#[allow(unused_imports)]
use provenance_sdk::{rule, verifies};
RUST

if ! cargo check --manifest-path "$fixture/Cargo.toml" --all-targets; then
  channel_failure "$channel" "the installed provenance-sdk macro reexports did not compile"
  exit 1
fi
if ! cargo metadata --locked --format-version 1 --manifest-path "$fixture/Cargo.toml" |
  jq -e --arg version "$version" \
    '.packages | any(.name == "provenance-sdk" and .version == $version)'
then
  channel_failure "$channel" "Cargo.lock did not resolve provenance-sdk $version exactly"
  exit 1
fi
printf 'crates.io: provenance-cli %s passed the installation smoke test\n' "$version"
