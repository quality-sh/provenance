import { createServer } from "node:http";
import { readFileSync, renameSync, writeFileSync } from "node:fs";

const [fixturePath, readyPath, countPath] = process.argv.slice(2);
const fixture = readFileSync(fixturePath);
const requests = [];

const server = createServer((request, response) => {
  requests.push({ method: request.method, url: request.url });
  writeFileSync(countPath, JSON.stringify(requests));
  if (request.method !== "GET" || request.url !== "/ASD-STE100_ISSUE9.pdf") {
    response.writeHead(404).end();
    return;
  }

  response.writeHead(200, {
    "content-length": fixture.length,
    "content-type": "application/pdf",
  });
  response.end(fixture);
});

server.listen(0, "127.0.0.1", () => {
  const { port } = server.address();
  const stagedReadyPath = `${readyPath}.${process.pid}.partial`;
  writeFileSync(stagedReadyPath, JSON.stringify({ port }));
  renameSync(stagedReadyPath, readyPath);
});

process.on("SIGTERM", () => server.close(() => process.exit(0)));
