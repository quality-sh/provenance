import { readFile } from 'node:fs/promises';
import openapiTS, { astToString } from 'openapi-typescript';
import { typescriptClient } from './templates.mjs';
import { validators } from './validators.mjs';
import { typescriptSchema } from './typescript-schema.mjs';

export async function typescriptFiles(document) {
  return {
    'client.ts': typescriptClient(document),
    'schema.ts': astToString(await openapiTS(typescriptSchema(document), { defaultNonNullable: false })),
    'runtime.ts': await readFile(new URL('./templates/http-runtime.ts', import.meta.url), 'utf8'),
    ...await validators(document),
  };
}

export { effectFiles } from './effect.mjs';
