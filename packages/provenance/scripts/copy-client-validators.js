import { cpSync, copyFileSync, mkdirSync } from 'node:fs';
const destination = process.argv[2] ?? 'dist';
const root = new URL('../', import.meta.url);
const output = new URL(`${destination}/generated/`, root);
mkdirSync(output, { recursive: true });
cpSync(new URL('src/generated/validators/', root), new URL('validators/', output), { recursive: true });
for (const name of ['effect-validators.mjs', 'effect-validators.d.mts']) {
  copyFileSync(new URL(`src/generated/${name}`, root), new URL(name, output));
}
