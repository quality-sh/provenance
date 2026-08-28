import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export function createPackedCommands({ temporary, npmCli, isolatedCache }) {
  const npxCli = join(dirname(npmCli), "npx-cli.js");
  let staleGlobalOrdinal = 0;

  function provenance(args, cwd) {
    const { PROVENANCE_BIN: _removed, ...environment } = process.env;
    return execFileSync(process.execPath, [npxCli, "--no", "provenance", ...args], {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...environment,
        npm_config_offline: "true",
        npm_config_cache: isolatedCache,
        npm_config_update_notifier: "false",
      },
    });
  }

  function verifyGeneratedCommandIgnoresStaleGlobal(cwd) {
    const sentinelDirectory = join(temporary, `stale-global-${staleGlobalOrdinal++}`);
    const sentinel = join(
      sentinelDirectory,
      process.platform === "win32" ? "provenance.cmd" : "provenance",
    );
    const selected = join(sentinelDirectory, "selected");
    mkdirSync(sentinelDirectory);
    writeFileSync(
      sentinel,
      process.platform === "win32"
        ? `@echo stale>${selected}\r\n@exit /b 42\r\n`
        : `#!/bin/sh\nprintf stale > '${selected}'\nexit 42\n`,
    );
    if (process.platform !== "win32") chmodSync(sentinel, 0o755);

    const agents = readFileSync(join(cwd, "AGENTS.md"), "utf8");
    assert.match(agents, /`npx --no provenance prime --quiet`/);
    const output = execFileSync(process.execPath, [
      npxCli,
      "--no",
      "provenance",
      "check",
      "--repo",
      ".",
      "--format",
      "json",
    ], {
      cwd,
      encoding: "utf8",
      env: {
        ...process.env,
        PATH: `${sentinelDirectory}${process.platform === "win32" ? ";" : ":"}${process.env.PATH}`,
        npm_config_offline: "true",
        npm_config_cache: isolatedCache,
        npm_config_update_notifier: "false",
      },
    });
    assert.equal(JSON.parse(output).status, "ok");
    assert.throws(() => readFileSync(selected), /ENOENT/);
  }

  return { provenance, verifyGeneratedCommandIgnoresStaleGlobal };
}
