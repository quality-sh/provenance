#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=.github/scripts/release-smoke/lib.sh
source "$script_directory/lib.sh"

version=${1-}
validate_version "$version"
channel="Deno"
smoke_root=$(make_smoke_directory deno-registry)
trap 'rm -rf -- "$smoke_root"' EXIT

export HOME="$smoke_root/home"
export NPM_CONFIG_CACHE="$smoke_root/npm-cache"
export NPM_CONFIG_USERCONFIG="$smoke_root/npmrc"
mkdir -p "$HOME" "$NPM_CONFIG_CACHE"
: > "$NPM_CONFIG_USERCONFIG"

fixture="$smoke_root/deno-fixture"
target_manifest="$script_directory/../../release-targets.json"
retry_channel \
  "$channel" npm_release_packages_are_visible "$target_manifest" "$version"
mkdir -p "$fixture"
cat > "$fixture/package.json" <<JSON
{
  "name": "provenance-deno-release-smoke",
  "private": true,
  "version": "0.0.0",
  "packageManager": "deno@$(deno --version | sed -n '1s/^deno //p')"
}
JSON

(
  cd "$fixture"
  npx --yes "@quality-sh/create-provenance@$version" \
    --path "$fixture" --package-manager deno
)

node - "$fixture" "$version" <<'JS'
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

const [root, version] = process.argv.slice(2);
const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
if (manifest.devDependencies?.["@quality-sh/provenance"] !==
    `npm:@quality-sh/provenance@${version}`) {
  process.exit(1);
}
const state = JSON.parse(
  readFileSync(join(root, ".provenance", "state", "manifest.json"), "utf8"),
);
if (state.scopes?.[0]?.path_prefix !== ".") process.exit(1);
JS

assert_initialized_repository "$channel" "$fixture"
assert_provenance_check \
  "$channel" "$fixture" "$fixture/node_modules/.bin/provenance"
assert_binary_version \
  "$channel" "$version" provenance "$fixture/node_modules/.bin/provenance"
printf 'Deno: @quality-sh/create-provenance %s used the current npm engine\n' "$version"
