#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=.github/scripts/release-smoke/lib.sh
source "$script_directory/lib.sh"

version=${1-}
validate_version "$version"
channel="npm"
smoke_root=$(make_smoke_directory npm-registry)
trap 'rm -rf -- "$smoke_root"' EXIT

export HOME="$smoke_root/home"
export NPM_CONFIG_CACHE="$smoke_root/npm-cache"
export NPM_CONFIG_USERCONFIG="$smoke_root/npmrc"
mkdir -p "$HOME" "$NPM_CONFIG_CACHE"
: > "$NPM_CONFIG_USERCONFIG"

fixture="$smoke_root/npm-fixture"

npm_packages_are_visible() {
  npm view "@quality-sh/create-provenance@$version" version >/dev/null &&
    npm view "@quality-sh/provenance@$version" version >/dev/null &&
    npm view "@quality-sh/provenance-linux-x64-gnu@$version" version >/dev/null
}

initialize_npm_fixture() {
  (
    cd "$fixture"
    npx --yes "@quality-sh/create-provenance@$version" \
      --path "$fixture" --package-manager npm
  )
}

assert_npm_sdk_requirement() {
  if ! node - "$fixture" "$version" <<'JS'
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

const [root, version] = process.argv.slice(2);
const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const lockfile = JSON.parse(readFileSync(join(root, "package-lock.json"), "utf8"));
if (manifest.devDependencies?.["@quality-sh/provenance"] !== version) process.exit(1);
if (lockfile.packages?.["node_modules/@quality-sh/provenance"]?.version !== version) process.exit(1);
JS
  then
    channel_failure "$channel" \
      "npm project does not require and lock @quality-sh/provenance $version exactly"
    return 1
  fi
}

retry_channel "$channel" npm_packages_are_visible
mkdir -p "$fixture"
cat > "$fixture/package.json" <<'JSON'
{
  "name": "provenance-release-smoke",
  "private": true,
  "version": "0.0.0"
}
JSON

if ! initialize_npm_fixture; then
  channel_failure "$channel" "npm initialization failed after all packages became visible"
  exit 1
fi
assert_npm_sdk_requirement
assert_initialized_repository "$channel" "$fixture"
capture_initialized_repository "$channel" "$fixture" "$smoke_root/first-init"

if ! initialize_npm_fixture; then
  channel_failure "$channel" "the second npm initialization failed"
  exit 1
fi
assert_npm_sdk_requirement
assert_initialized_repository "$channel" "$fixture"
capture_initialized_repository "$channel" "$fixture" "$smoke_root/second-init"
assert_initialized_snapshots_equal \
  "$channel" "$smoke_root/first-init" "$smoke_root/second-init"
assert_provenance_check \
  "$channel" "$fixture" "$fixture/node_modules/.bin/provenance"

assert_binary_version \
  "$channel" "$version" provenance "$fixture/node_modules/.bin/provenance"
printf 'npm: @quality-sh/create-provenance %s passed the installation smoke test\n' "$version"
