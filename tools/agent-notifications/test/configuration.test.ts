import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { run as codex } from "../codex";
import { run as claude } from "../claude";

// Implementation aid: preserve which error wins when configuration has several faults.
for (const [name, run] of [["Codex", codex], ["Claude", claude]] as const) {
  test(`${name} validates every selection field before filesystem access`, async () => {
    const directory = mkdtempSync(join(tmpdir(), "provenance-config-"));
    try {
      await expect(run({ clientEndpoint: "ws://127.0.0.1:1", threadId: "thread",
        repository: join(directory, "missing"), scope: 0, rootId: "" })).rejects.toThrow("Configuration requires scope");
    } finally { rmSync(directory, { recursive: true, force: true }); }
  });
}
