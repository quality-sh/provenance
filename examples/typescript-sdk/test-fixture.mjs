// Explicit source-checkout harness, not an SDK or production host launcher.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { startFixtureHost } from "../../packages/provenance/scripts/fixture-host.js";

const root = fileURLToPath(new URL(".", import.meta.url));
const fixture = await startFixtureHost({ root, repositoryId: "typescript-example" });
try {
  for (const entry of ["apply.js", "share-links.test.js"]) {
    execFileSync(process.execPath, [fileURLToPath(new URL(`./dist/${entry}`, import.meta.url))], {
      cwd: root,
      env: { ...process.env, ...fixture.environment },
      stdio: "inherit",
    });
  }
} finally { await fixture.close(); }
