#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib.sh
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

retry_channel "$channel" curl --fail --silent --show-error --location \
  --output "$smoke_root/SHA256SUMS" "$base_url/SHA256SUMS"
retry_channel "$channel" curl --fail --silent --show-error --location \
  --output "$smoke_root/$archive" "$base_url/$archive"

expected=$(expected_checksum "$smoke_root/SHA256SUMS" "$archive")
actual=$(sha256_file "$smoke_root/$archive")
if [[ "$actual" != "$expected" ]]; then
  channel_failure "$channel" "SHA-256 mismatch for $archive"
  exit 1
fi

extracted="$smoke_root/extracted"
mkdir -p "$extracted"
case "$archive_format" in
  tar.gz)
    if ! tar -xzf "$smoke_root/$archive" -C "$extracted"; then
      channel_failure "$channel" "could not extract $archive"
      exit 1
    fi
    ;;
  zip)
    if ! unzip -q "$smoke_root/$archive" -d "$extracted"; then
      channel_failure "$channel" "could not extract $archive"
      exit 1
    fi
    ;;
esac

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
  assert_binary_version "$channel" "$version" "$path"
done

printf 'GitHub archive: %s passed checksum and execution smoke tests\n' "$archive"
