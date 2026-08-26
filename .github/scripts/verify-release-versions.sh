#!/usr/bin/env bash
set -euo pipefail

tag=${1:?usage: verify-release-versions.sh <tag>}
version=${tag#v}

cargo metadata --locked --no-deps --format-version 1 |
  jq -e --arg version "$version" \
    '.packages | length > 0 and all(.[]; .version == $version)' >/dev/null

cargo metadata --locked --manifest-path examples/rust-sdk/Cargo.toml --format-version 1 |
  jq -e --arg version "$version" \
    '[.packages[] | select(.name | startswith("provenance-"))] |
     length > 0 and all(.[]; .version == $version)' >/dev/null

node - "$version" <<'NODE'
const version = process.argv[2];
const read = (path) => require(`${process.cwd()}/${path}`);
const fail = (message) => {
  console.error(message);
  process.exitCode = 1;
};
const expectVersion = (actual, location) => {
  if (actual !== version) fail(`${location} is ${actual ?? "missing"}; expected ${version}`);
};

const sdkManifest = read("packages/provenance/package.json");
const sdkLock = read("packages/provenance/package-lock.json");
const initializerManifest = read("packages/create-provenance/package.json");
const initializerLock = read("packages/create-provenance/package-lock.json");
const exampleLock = read("examples/typescript-sdk/package-lock.json");
const expectedPlatformPackages = [
  "@quality-sh/provenance-darwin-arm64",
  "@quality-sh/provenance-darwin-x64",
  "@quality-sh/provenance-linux-x64-gnu",
  "@quality-sh/provenance-win32-x64-msvc",
];

const expectPlatformPackages = (dependencies, location) => {
  const actual = Object.keys(dependencies).sort();
  if (JSON.stringify(actual) !== JSON.stringify(expectedPlatformPackages)) {
    fail(`${location} platform package set is ${actual.join(", ")}; expected ${expectedPlatformPackages.join(", ")}`);
  }
};

expectVersion(sdkManifest.version, "packages/provenance/package.json");
expectPlatformPackages(sdkManifest.optionalDependencies, "packages/provenance/package.json");
for (const [name, dependencyVersion] of Object.entries(sdkManifest.optionalDependencies)) {
  expectVersion(dependencyVersion, `${name} optional dependency`);
}
expectVersion(sdkLock.version, "packages/provenance/package-lock.json");
expectVersion(sdkLock.packages[""].version, "packages/provenance lock root");
expectPlatformPackages(sdkLock.packages[""].optionalDependencies, "packages/provenance lock root");
const lockedPlatformPackages = Object.fromEntries(
  Object.entries(sdkLock.packages)
    .filter(([path]) => path.startsWith("node_modules/@quality-sh/provenance-"))
    .map(([path, record]) => [path.slice("node_modules/".length), record]),
);
expectPlatformPackages(lockedPlatformPackages, "packages/provenance lock records");
for (const [name, dependencyVersion] of Object.entries(sdkLock.packages[""].optionalDependencies)) {
  expectVersion(dependencyVersion, `${name} locked optional dependency`);
  expectVersion(sdkLock.packages[`node_modules/${name}`]?.version, `${name} lock record`);
}

expectVersion(initializerManifest.version, "packages/create-provenance/package.json");
expectVersion(initializerLock.version, "packages/create-provenance/package-lock.json");
expectVersion(initializerLock.packages[""].version, "packages/create-provenance lock root");

const linkedSdk = exampleLock.packages["../../packages/provenance"];
expectVersion(linkedSdk.version, "examples/typescript-sdk linked SDK");
expectPlatformPackages(linkedSdk.optionalDependencies, "examples/typescript-sdk linked SDK");
for (const [name, dependencyVersion] of Object.entries(linkedSdk.optionalDependencies)) {
  expectVersion(dependencyVersion, `${name} example lock dependency`);
}
NODE
