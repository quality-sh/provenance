#!/usr/bin/env node
// The package the quick start installs owns the `provenance` command, so
// `npx provenance` always reaches this file and never a package of the same
// name on the registry. It finds the engine and hands over the command line
// unchanged: every decision below this point belongs to Rust.
// @provenance rule: rule_sdk_package_supplies_engine
import { spawnSync } from "node:child_process";
import { resolveEnginePath } from "../dist/engine-path.js";

let engine;
try {
  engine = resolveEnginePath(process.env.PROVENANCE_BIN);
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(1);
}

const result = spawnSync(engine, process.argv.slice(2), { stdio: "inherit" });
if (result.error !== undefined) {
  process.stderr.write(
    `Provenance engine could not start at ${engine}. Check PROVENANCE_BIN if it is set, ` +
    `or run npm install @quality-sh/provenance without --omit=optional.\n`,
  );
  process.exit(1);
}
process.exit(result.status ?? 1);
