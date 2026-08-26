#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=.github/scripts/release-smoke/lib.sh
source "$script_directory/lib.sh"

version=${1-}
target=${2-}
archive_format=${3-}
validate_version "$version"

case "$target:$archive_format" in
  x86_64-unknown-linux-gnu:tar.gz | x86_64-apple-darwin:tar.gz | aarch64-apple-darwin:tar.gz)
    executable_suffix=
    ;;
  x86_64-pc-windows-msvc:zip)
    executable_suffix=.exe
    ;;
  *)
    channel_failure "GitHub archive" "unsupported smoke target: $target ($archive_format)"
    exit 2
    ;;
esac

channel="GitHub archive $target"
smoke_root=$(make_smoke_directory github-archive)
trap 'rm -rf -- "$smoke_root"' EXIT

tag="v$version"
package="provenance-$tag-$target"
archive="$package.$archive_format"
base_url="https://github.com/quality-sh/provenance/releases/download/$tag"

release_assets_are_visible() {
  curl --fail --silent --show-error --location --head \
    "$base_url/SHA256SUMS" >/dev/null &&
    curl --fail --silent --show-error --location --head \
      "$base_url/$archive" >/dev/null
}

retry_channel "$channel" release_assets_are_visible
if ! curl --fail --silent --show-error --location \
  --output "$smoke_root/SHA256SUMS" "$base_url/SHA256SUMS"
then
  channel_failure "$channel" "could not download SHA256SUMS after it became visible"
  exit 1
fi
if ! curl --fail --silent --show-error --location \
  --output "$smoke_root/$archive" "$base_url/$archive"
then
  channel_failure "$channel" "could not download $archive after it became visible"
  exit 1
fi

expected=$(expected_checksum "$smoke_root/SHA256SUMS" "$archive")
actual=$(sha256_file "$smoke_root/$archive")
if [[ "$actual" != "$expected" ]]; then
  channel_failure "$channel" "SHA-256 mismatch for $archive"
  exit 1
fi

extracted="$smoke_root/extracted"
mkdir -p "$extracted"
if ! command -v tar >/dev/null 2>&1; then
  channel_failure "$channel" "the runner does not provide the cross-platform tar extractor"
  exit 1
fi
if ! tar -xf "$smoke_root/$archive" -C "$extracted"; then
  channel_failure "$channel" "could not extract $archive with tar"
  exit 1
fi

for binary in provenance cargo-provenance; do
  if [[ "$archive_format" == zip ]]; then
    path="$extracted/$binary$executable_suffix"
  else
    path="$extracted/$package/$binary$executable_suffix"
  fi
  if [[ ! -f "$path" ]]; then
    channel_failure "$channel" "$archive does not contain $binary$executable_suffix"
    exit 1
  fi
  assert_binary_version "$channel" "$version" "$binary" "$path"
done

printf 'GitHub archive: %s passed checksum and execution smoke tests\n' "$archive"
