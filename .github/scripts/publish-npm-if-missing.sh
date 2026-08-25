#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 4 ]]; then
  echo "usage: publish-npm-if-missing.sh <package> <version> <archive> <tag>" >&2
  exit 2
fi

package_name="$1"
version="$2"
archive="$3"
npm_tag="$4"
package_spec="${package_name}@${version}"

if published_version=$(npm view "$package_spec" version --json 2>&1); then
  if [[ "$published_version" != "\"${version}\"" ]]; then
    echo "npm returned an unexpected version for ${package_spec}: ${published_version}" >&2
    exit 1
  fi
  echo "${package_spec} is already published; skipping it."
  exit 0
fi

if [[ "$published_version" == *E404* ]]; then
  npm publish "$archive" --access public --provenance --tag "$npm_tag"
  exit 0
fi

printf '%s\n' "$published_version" >&2
exit 1
