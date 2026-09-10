import { build } from 'esbuild';
import ts from 'typescript';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, readdir, copyFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const [rendererArg, sdkArg, outputArg] = process.argv.slice(2);
if (!rendererArg || !sdkArg || !outputArg) throw new Error('Usage: node build.ts WEB_ASSETS SDK_PACKAGE NEW_OUTPUT');
const renderer = resolve(rendererArg);
const sdk = resolve(sdkArg);
const output = resolve(outputArg);
const hash = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex');
async function files(directory: string): Promise<string[]> {
  const result: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) throw new Error('Asset links are not allowed');
    if (entry.isDirectory()) for (const file of await files(join(directory, entry.name))) result.push(`${entry.name}/${file}`);
    else if (entry.isFile()) result.push(entry.name);
    else throw new Error('Assets must be regular files');
  }
  return result.sort();
}
const declarations = join(renderer, 'types/browser/main.d.ts');
await readFile(declarations);
const sdkSchema = await readFile(join(sdk, 'dist/generated/schema.d.ts'));
const rendererInfo = JSON.parse(await readFile(join(renderer, 'build-info.json'), 'utf8'));
if (rendererInfo.sdkSchemaSha256 !== hash(sdkSchema)) throw new Error('Renderer and host require the same generated SDK contract');
const options: ts.CompilerOptions = {
  noEmit: true, strict: true, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler, allowImportingTsExtensions: true,
  types: [], paths: {
    'review-renderer': [declarations],
    '@quality-sh/provenance/client': [join(sdk, 'dist/client.d.ts')],
  },
};
const diagnostics = ts.getPreEmitDiagnostics(ts.createProgram([join(root, 'main.ts')], options));
if (diagnostics.length) {
  console.error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCanonicalFileName: file => file, getCurrentDirectory: () => root, getNewLine: () => '\n',
  }));
  throw new Error('Host typecheck failed');
}
await mkdir(output, { recursive: false });
const rendererFiles: Record<string, string> = {};
for (const file of await files(renderer)) {
  const bytes = await readFile(join(renderer, file));
  rendererFiles[file] = hash(bytes);
  if (file.startsWith('types/') || file === 'index.html') continue;
  await mkdir(dirname(join(output, file)), { recursive: true });
  await copyFile(join(renderer, file), join(output, file));
}
for (const file of ['index.html', 'host.css']) await copyFile(join(root, file), join(output, file));
await build({
  entryPoints: [join(root, 'main.ts')], outfile: join(output, 'host.js'), bundle: true,
  format: 'esm', platform: 'browser', target: 'es2022', metafile: true,
  alias: { '@quality-sh/provenance/client': join(sdk, 'dist/client.js') },
  plugins: [{ name: 'renderer', setup(builder) {
    builder.onResolve({ filter: /^review-renderer$/ }, () => ({ path: './review.js', external: true }));
  } }],
});
const outputFiles: Record<string, string> = {};
for (const file of await files(output)) outputFiles[file] = hash(await readFile(join(output, file)));
await writeFile(join(output, 'host-build-info.json'), JSON.stringify({
  formatVersion: 1, renderer: rendererInfo, rendererFiles,
  sdkVersion: JSON.parse(await readFile(join(sdk, 'package.json'), 'utf8')).version,
  sdkSchemaSha256: hash(sdkSchema), files: outputFiles,
}, null, 2) + '\n');
console.log(`Built host assets: ${output}`);
