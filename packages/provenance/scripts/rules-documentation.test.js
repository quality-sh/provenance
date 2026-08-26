import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const repositoryRoot = new URL("../../..", import.meta.url);

function repositoryFile(path) {
  return readFileSync(new URL(path, repositoryRoot), "utf8");
}

test("the installed SDK documents its rule binding subpath", () => {
  const packageReadme = repositoryFile("packages/provenance/README.md");
  const bindingGuide = repositoryFile("docs/rule-bindings.md");

  for (const documentation of [packageReadme, bindingGuide]) {
    assert.match(
      documentation,
      /from ["']@quality-sh\/provenance\/rules["']/,
    );
    assert.match(documentation, /npm install @quality-sh\/provenance/);
  }
});

test("documentation does not direct users to the removed helper package", () => {
  for (const path of [
    "packages/provenance/README.md",
    "docs/rule-bindings.md",
    "docs/cli.md",
    "docs/typescript-sdk-poc.md",
  ]) {
    const documentation = repositoryFile(path);
    assert.doesNotMatch(documentation, /@provenance\/rules|provenance-rules-js/);
  }
});
