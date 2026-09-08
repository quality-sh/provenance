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

const arguments_ = process.argv.slice(2);
const commandIndex = arguments_.findIndex((argument) => argument !== "--quiet");
if (arguments_[commandIndex] === "init" && !arguments_.includes("--invocation-channel")) {
  const manager = process.env.PROVENANCE_PACKAGE_MANAGER ??
    process.env.npm_config_user_agent?.match(/^([^/\s]+)\//)?.[1] ?? "npm";
  arguments_.push("--invocation-channel", "typescript", "--package-manager", manager);
}
const result = spawnSync(engine, arguments_, { stdio: "inherit" });
if (result.error !== undefined) {
  process.stderr.write(
    `Provenance engine could not start at ${engine}. Check PROVENANCE_BIN if it is set, ` +
    `or install the development dependency again with optional dependencies enabled.\n`,
  );
  process.exit(1);
}
process.exit(result.status ?? 1);
