#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
source "$script_directory/lib.sh"

version=${1-}
validate_version "$version"
channel="npm"
smoke_root=$(make_smoke_directory npm-registry)
trap 'rm -rf -- "$smoke_root"' EXIT

export HOME="$smoke_root/home"
export npm_config_cache="$smoke_root/npm-cache"
export npm_config_userconfig="$smoke_root/npmrc"
mkdir -p "$HOME" "$npm_config_cache"
: > "$npm_config_userconfig"

fixture="$smoke_root/npm-fixture"

npm_packages_are_visible() {
  npm view "@quality-sh/create-provenance@$version" version >/dev/null &&
    npm view "@quality-sh/provenance@$version" version >/dev/null &&
    npm view "@quality-sh/provenance-linux-x64-gnu@$version" version >/dev/null
}

initialize_npm_fixture() {
  rm -rf -- "$fixture"
  mkdir -p "$fixture"
  cat > "$fixture/package.json" <<'JSON'
{
  "name": "provenance-release-smoke",
  "private": true,
  "version": "0.0.0"
}
JSON
  (
    cd "$fixture"
    npx --yes "@quality-sh/create-provenance@$version" \
      --path "$fixture" --package-manager npm
  )
}

retry_channel "$channel" npm_packages_are_visible
retry_channel "$channel" initialize_npm_fixture

if [[ ! -f "$fixture/.provenance/state/manifest.json" ]]; then
  channel_failure "$channel" "initializer did not create Provenance state"
  exit 1
fi

installed_version=$(node -e \
  'process.stdout.write(require(process.argv[1]).version)' \
  "$fixture/node_modules/@quality-sh/provenance/package.json")
if [[ "$installed_version" != "$version" ]]; then
  channel_failure "$channel" \
    "initializer installed @quality-sh/provenance $installed_version instead of $version"
  exit 1
fi

assert_binary_version "$channel" "$version" "$fixture/node_modules/.bin/provenance"
printf 'npm: @quality-sh/create-provenance %s passed the installation smoke test\n' "$version"
