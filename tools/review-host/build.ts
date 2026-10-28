import { build } from 'esbuild';
import ts from 'typescript';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, readdir, copyFile } from 'node:fs/promises';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const [rendererArg, outputArg, pinArg] = process.argv.slice(2);
if (!rendererArg || !outputArg) throw new Error('Usage: node build.ts WEB_ASSETS NEW_OUTPUT [PIN]');
const renderer = resolve(rendererArg);
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
const rendererInfo = JSON.parse(await readFile(join(renderer, 'build-info.json'), 'utf8'));
if (pinArg) {
  const pin = JSON.parse(await readFile(resolve(pinArg), 'utf8'));
  if (rendererInfo.formatVersion !== 1 || rendererInfo.commit !== pin.commit || rendererInfo.dirty !== false) {
    throw new Error('Renderer identity does not match the clean pinned commit');
  }
}
const options: ts.CompilerOptions = {
  noEmit: true, strict: true, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler, allowImportingTsExtensions: true,
  types: [], paths: {
    'review-renderer': [declarations],
  },
};
// The renderer's declarations import its own dependencies (effect, the SDK), which the host does
// not install and provenance-web checks. Only a missing package in those files is ignored, so a
// missing renderer file or a wrong mount option type still fails. Bead provenance-6470 moves the
// mount option types into a module that imports nothing; then this filter can go.
const rendererTypes = join(renderer, 'types') + sep;
const missingRendererDependency = (diagnostic: ts.Diagnostic) => {
  if (diagnostic.code !== 2307 || !diagnostic.file || diagnostic.start === undefined) return false;
  if (!resolve(diagnostic.file.fileName).startsWith(rendererTypes)) return false;
  const specifier = diagnostic.file.text.slice(diagnostic.start + 1, diagnostic.start + (diagnostic.length ?? 0) - 1);
  return !specifier.startsWith('.') && !specifier.startsWith('/');
};
const diagnostics = ts.getPreEmitDiagnostics(ts.createProgram([join(root, 'main.ts')], options))
  .filter(diagnostic => !missingRendererDependency(diagnostic));
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
  plugins: [{ name: 'renderer', setup(builder) {
    builder.onResolve({ filter: /^review-renderer$/ }, () => ({ path: './review.js', external: true }));
  } }],
});
const outputFiles: Record<string, string> = {};
for (const file of await files(output)) outputFiles[file] = hash(await readFile(join(output, file)));
await writeFile(join(output, 'host-build-info.json'), JSON.stringify({
  formatVersion: 1, renderer: rendererInfo, rendererFiles,
  files: outputFiles,
}, null, 2) + '\n');
console.log(`Built host assets: ${output}`);
