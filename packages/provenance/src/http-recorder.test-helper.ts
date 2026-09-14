import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { createServer } from "node:http";
import { COMPATIBILITY, type components } from "./generated/client.js";
import { STATE_SCHEMA_VERSION } from "./protocol.js";
import type { ConfigureOptions } from "./settings.js";

type Schemas = components["schemas"];
type VerificationRun = Schemas["BeginVerificationSuccess"]["data"];
type RecordedRequest = { command: string; input: unknown; context: { repository: string; scope: string } };

const routes: ReadonlyArray<[RegExp, string]> = [
  [/^\/authoring-changes$/, "apply"],
  [/^\/authoring-plans$/, "plan"],
  [/^\/verification-runs\/begin-verification$/, "begin-verification"],
  [/^\/verification-runs\/[^/]+\/complete-verification$/, "complete-verification"],
];

/** Records SDK orchestration over the v2 HTTP resource surface. */
export async function recordingHost(responses: Readonly<Record<string, unknown>> = {}): Promise<{
  settings: ConfigureOptions;
  requests: () => RecordedRequest[];
  close: () => Promise<void>;
}> {
  const recorded: RecordedRequest[] = [];
  const runs = new Map<string, VerificationRun>();
  const bearer = randomUUID();
  const identity = { repository: "recorder", scope: "default" };
  const server = createServer(async (request, response) => {
    response.setHeader("content-type", "application/json");
    if (request.headers.authorization !== `Bearer ${bearer}`) {
      response.writeHead(401).end(JSON.stringify({ error: { kind: "unauthenticated" }, meta: {} }));
      return;
    }
    if (request.method === "GET" && request.url === "/metadata") {
      response.end(JSON.stringify({ data: {
        compatibility: COMPATIBILITY,
        package: { name: "provenance", version: "test-recorder" },
        contract_digest: "test-recorder",
        ...identity,
      }, meta: {} }));
      return;
    }
    const path = request.url?.split("?", 1)[0] ?? "";
    const command = routes.find(([pattern]) => pattern.test(path))?.[1];
    if (request.method !== "POST" || command === undefined) {
      response.writeHead(404).end(JSON.stringify({ error: { kind: "unknown_operation" }, meta: {} }));
      return;
    }
    try {
      const chunks: Buffer[] = [];
      for await (const chunk of request) chunks.push(Buffer.from(chunk));
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as { data: unknown };
      recorded.push({ command, input: body.data, context: identity });
      let result: unknown;
      if (Object.hasOwn(responses, command)) {
        result = responses[command];
      } else if (command === "plan" || command === "apply") {
        const input = body.data as { declared_by: string };
        result = {
          declared_by: input.declared_by, created: 0, updated: 0,
          moved: 0, deleted: 0, conflicts: 0, unchanged: 0, resources: [],
          ...(command === "plan" ? { affected_rules: [] } : {}),
        };
      } else if (command === "begin-verification") {
        const input = body.data as Schemas["BeginVerificationRequest"]["data"];
        const run: VerificationRun = {
          schema_version: STATE_SCHEMA_VERSION, scope_id: identity.scope,
          id: `run_${input.key}`, binding_id: `verification_binding_${input.key}`,
          rule_id: input.rule ?? "rule_expiry", method: input.method,
          declared_by: input.declared_by, status: "running", started_at: 1,
          file: input.file, symbol: input.symbol, commit: input.commit,
          completed_at: null, error: null,
        };
        runs.set(run.id, run);
        result = run;
      } else {
        const input = body.data as Schemas["CompleteVerificationRequest"]["data"];
        const runId = path.split("/")[2];
        const run = runs.get(runId);
        if (!run) throw new Error(`Unknown recorder run: ${runId}`);
        result = { ...run, status: input.status, completed_at: 2, error: input.error };
      }
      response.end(JSON.stringify({ data: result, meta: {} }));
    } catch (error) {
      response.writeHead(500).end(JSON.stringify({ error: { kind: "internal", message: String(error) }, meta: {} }));
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Missing recorder address");
  let closed: Promise<void> | undefined;
  return {
    settings: { endpoint: `http://127.0.0.1:${address.port}`, bearer, repositoryId: identity.repository, scope: identity.scope },
    requests: () => [...recorded],
    close: () => closed ??= new Promise<void>((resolve, reject) => {
      server.close((error) => error ? reject(error) : resolve());
      server.closeAllConnections();
    }),
  };
}
