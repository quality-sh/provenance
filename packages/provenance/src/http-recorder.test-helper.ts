import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { createServer } from "node:http";
import { PROTOCOL_VERSION, type components } from "./generated/client.js";
import { STATE_SCHEMA_VERSION } from "./protocol.js";
import type { ConfigureOptions } from "./settings.js";

type Schemas = components["schemas"];
type RecordedRequest = { command: string; input: unknown; context?: unknown };

/** Records SDK orchestration over HTTP; persisted behavior uses the Store fixture. */
export async function recordingHost(responses: Readonly<Record<string, unknown>> = {}): Promise<{
  settings: ConfigureOptions;
  requests: () => RecordedRequest[];
  close: () => Promise<void>;
}> {
  const recorded: RecordedRequest[] = [];
  const runs = new Map<string, Schemas["BeginVerificationSuccessOutput"]>();
  const bearer = randomUUID();
  const server = createServer(async (request, response) => {
    response.setHeader("content-type", "application/json");
    if (request.headers.authorization !== `Bearer ${bearer}`) {
      response.writeHead(401).end("{}");
      return;
    }
    if (request.method === "GET" && request.url === "/metadata") {
      response.end(JSON.stringify({ protocol_version: PROTOCOL_VERSION, engine_version: "test-recorder" }));
      return;
    }
    const prefix = `/v${PROTOCOL_VERSION}/operations/`;
    if (request.method !== "POST" || !request.url?.startsWith(prefix)) {
      response.writeHead(404).end("{}");
      return;
    }
    try {
      const chunks: Buffer[] = [];
      for await (const chunk of request) chunks.push(Buffer.from(chunk));
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      const command = request.url.slice(prefix.length);
      recorded.push({ command, input: body.request, context: body.context });
      let result: unknown;
      if (Object.hasOwn(responses, command)) {
        result = responses[command];
      } else if (command === "plan" || command === "apply") {
        result = {
          declared_by: body.request.declared_by, created: 0, updated: 0,
          moved: 0, deleted: 0, conflicts: 0, unchanged: 0, resources: [],
          ...(command === "plan" ? { affected_rules: [] } : {}),
        };
      } else if (command === "begin-verification") {
        const input = body.request as Schemas["BeginVerificationRequestInput"]["request"];
        const run: Schemas["BeginVerificationSuccessOutput"] = {
          schema_version: STATE_SCHEMA_VERSION, scope_id: body.context.scope,
          id: `run_${input.key}`, binding_id: `verification_binding_${input.key}`,
          rule_id: input.rule ?? "rule_expiry", method: input.method,
          declared_by: input.declared_by, status: "running", started_at: 1,
          file: input.file, symbol: input.symbol,
        };
        runs.set(run.id, run);
        result = run;
      } else if (command === "complete-verification") {
        const input = body.request as Schemas["CompleteVerificationRequestInput"]["request"];
        const run = runs.get(input.run);
        if (!run) throw new Error(`Unknown recorder run: ${input.run}`);
        result = { ...run, status: input.status, completed_at: 2, error: input.error };
      } else {
        throw new Error(`No recorder response for ${command}`);
      }
      response.end(JSON.stringify(result));
    } catch (error) {
      response.writeHead(500).end(JSON.stringify({ recorder_error: String(error) }));
    }
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Missing recorder address");
  let closed: Promise<void> | undefined;
  return {
    settings: { endpoint: `http://127.0.0.1:${address.port}`, bearer, repositoryId: "recorder", scope: "default" },
    requests: () => [...recorded],
    close: () => closed ??= new Promise<void>((resolve, reject) => {
      server.close((error) => error ? reject(error) : resolve());
      server.closeAllConnections();
    }),
  };
}
