#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$script_directory/lib.sh"

version=${1-}
validate_version "$version"
channel="crates.io"
smoke_root=$(make_smoke_directory cargo-registry)
trap 'rm -rf -- "$smoke_root"' EXIT

export CARGO_HOME="$smoke_root/cargo-home"
mkdir -p "$CARGO_HOME"
export PATH="$CARGO_HOME/bin:$PATH"

retry_channel "$channel" \
  cargo install --locked provenance-cli --version "=$version"

for binary in provenance cargo-provenance; do
  if ! command -v "$binary" >/dev/null 2>&1; then
    channel_failure "$channel" "provenance-cli $version did not install $binary"
    exit 1
  fi
  assert_binary_version "$channel" "$version" "$binary"
done

fixture="$smoke_root/rust-fixture"
mkdir -p "$fixture"
if ! cargo init --lib --name provenance_release_smoke "$fixture"; then
  channel_failure "$channel" "cargo init failed for the Rust fixture"
  exit 1
fi
if ! (
  cd "$fixture"
  cargo provenance init
  cargo provenance init
); then
  channel_failure "$channel" "cargo provenance init failed for the Rust fixture"
  exit 1
fi

mkdir -p "$fixture/tests"
cat > "$fixture/tests/sdk_macros.rs" <<'RUST'
use provenance_sdk::{rule, verifies};

#[rule("rule_release_smoke")]
fn installed_sdk_macro() {}

#[test]
#[verifies("rule_release_smoke", examples)]
fn sdk_verification_macro_compiles() {
    installed_sdk_macro();
}
RUST

if ! cargo check --manifest-path "$fixture/Cargo.toml" --all-targets; then
  channel_failure "$channel" "the installed provenance-sdk macro reexports did not compile"
  exit 1
fi
printf 'crates.io: provenance-cli %s passed the installation smoke test\n' "$version"
