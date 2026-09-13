import { createHash } from 'node:crypto';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

export const generatedDirectories = [
  'contracts/operations',
  'packages/provenance/src/generated',
  'crates/provenance-http-client/src/generated',
];
export const generatedPaths = [...generatedDirectories, 'packages/provenance/src/engine-packages.ts'];
const receipt = 'contracts/operations/.generation.json';
const crates = ['provenance-core', 'provenance-store', 'provenance-scanner', 'provenance-ste100', 'provenance-macros', 'provenance-codegen'];
const digest = value => createHash('sha256').update(value).digest('hex');

async function files(root, path) {
  let entries;
  try { entries = await readdir(join(root, path), { withFileTypes: true }); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
  const result = [];
  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name, 'en'))) {
    const name = `${path}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await files(root, name));
    else if (entry.isFile()) result.push(name);
    else throw new Error(`Unexpected non-file in generation inputs or outputs: ${name}`);
  }
  return result;
}

async function inputs(root) {
  const paths = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'];
  for (const crate of crates) {
    paths.push(`crates/${crate}/Cargo.toml`, ...await files(root, `crates/${crate}/src`));
  }
  // Generator dependencies and templates matter; its tests and documentation do not.
  const toolPaths = ['generate.mjs', 'cargo.mjs', 'artifacts.mjs', 'inventory.mjs', 'typescript.mjs', 'effect.mjs', 'typescript-schema.mjs', 'validators.mjs', 'templates.mjs', 'package.json', 'package-lock.json'];
  paths.push(...toolPaths.map(path => `tools/operation-codegen/${path}`), ...await files(root, 'tools/operation-codegen/templates'));
  const values = [];
  for (const path of paths.sort()) {
    try { values.push([path, digest((await readFile(join(root, path), 'utf8')).replaceAll('\r\n', '\n'))]); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
  return digest(JSON.stringify(values));
}

async function outputs(root) {
  const result = {};
  for (const directory of generatedDirectories) {
    for (const path of await files(root, directory)) {
      if (path !== receipt) result[path] = digest(await readFile(join(root, path)));
    }
  }
  return result;
}

export async function recordGeneration(sourceRoot, outputRoot) {
  const manifest = { version: 1, inputs: await inputs(sourceRoot), outputs: await outputs(outputRoot) };
  await mkdir(dirname(join(outputRoot, receipt)), { recursive: true });
  await writeFile(join(outputRoot, receipt), `${JSON.stringify(manifest, null, 2)}\n`);
}

export async function generationIsCurrent(root) {
  let manifest;
  try { manifest = JSON.parse(await readFile(join(root, receipt), 'utf8')); }
  catch (error) { if (error.code === 'ENOENT' || error instanceof SyntaxError) return false; throw error; }
  return manifest?.version === 1 && manifest.inputs === await inputs(root)
    && JSON.stringify(manifest.outputs) === JSON.stringify(await outputs(root));
}
