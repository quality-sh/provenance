import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { delimiter, join } from "node:path";

// The command that `provenance init` writes for an npm project. It names the
// bin file of the installed package, so npm, its npx cache and PATH cannot
// supply a different program.
export const npmCommand = "node node_modules/@quality-sh/provenance/bin/provenance.mjs";
export const packageBin = npmCommand.slice("node ".length);

export function createPackedCommands({ temporary, isolatedCache }) {
  let staleGlobalOrdinal = 0;

  function provenance(args, cwd) {
    const { PROVENANCE_BIN: _removed, ...environment } = process.env;
    return execFileSync(process.execPath, [packageBin, ...args], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...environment, ...npmEnvironment() },
    });
  }

  function npmEnvironment() {
    return {
      npm_config_offline: "true",
      npm_config_cache: isolatedCache,
      npm_config_update_notifier: "false",
    };
  }

  function createStaleGlobal() {
    const directory = join(temporary, `stale-global-${staleGlobalOrdinal++}`);
    const sentinel = join(
      directory,
      process.platform === "win32" ? "provenance.cmd" : "provenance",
    );
    const selected = join(directory, "selected");
    mkdirSync(directory);
    writeFileSync(
      sentinel,
      process.platform === "win32"
        ? `@echo stale>${selected}\r\n@exit /b 42\r\n`
        : `#!/bin/sh\nprintf stale > '${selected}'\nexit 42\n`,
    );
    if (process.platform !== "win32") chmodSync(sentinel, 0o755);
    return { directory, selected };
  }

  // Runs the written command through the shell of the platform, with a stale
  // `provenance` first on PATH. Windows spells the PATH key in any case.
  function runWritten(cwd, args, staleGlobal) {
    const environment = Object.fromEntries(
      Object.entries(process.env).filter(([key]) => key.toUpperCase() !== "PATH"),
    );
    return spawnSync(`${npmCommand} ${args}`, {
      cwd,
      encoding: "utf8",
      shell: true,
      env: {
        ...environment,
        PATH: `${staleGlobal.directory}${delimiter}${process.env.PATH}`,
        ...npmEnvironment(),
      },
    });
  }

  function verifyGeneratedCommandIgnoresStaleGlobal(cwd) {
    const staleGlobal = createStaleGlobal();
    const agents = readFileSync(join(cwd, "AGENTS.md"), "utf8");
    assert.ok(agents.includes(`\`${npmCommand} prime --quiet\``), agents);
    const check = runWritten(cwd, "check --repo . --format json", staleGlobal);
    assert.equal(check.status, 0, check.stderr);
    const categories = JSON.parse(check.stdout).categories;
    assert.equal(categories.length, 3);
    assert.ok(categories.every(category => category.status === "passed"));
    assert.throws(() => readFileSync(staleGlobal.selected), /ENOENT/);

    // Without the package, the written command fails and runs nothing else.
    const missing = join(staleGlobal.directory, "missing-package");
    mkdirSync(missing);
    writeFileSync(join(missing, "package.json"), '{"name":"missing","private":true}\n');
    const version = runWritten(missing, "--version", staleGlobal);
    assert.notEqual(version.status, 0, `${version.stdout}${version.stderr}`);
    assert.throws(() => readFileSync(staleGlobal.selected), /ENOENT/);
  }

  return { provenance, verifyGeneratedCommandIgnoresStaleGlobal };
}
