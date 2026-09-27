import { readFile } from 'node:fs/promises';
import openapiTS, { astToString } from 'openapi-typescript';
import { typescriptClient } from './templates.mjs';
import { operationValidators } from './validators.mjs';
import { typescriptSchema } from './typescript-schema.mjs';

export async function typescriptFiles(document, compatibility) {
  return {
    'client.ts': typescriptClient(document, compatibility),
    'schema.ts': astToString(await openapiTS(typescriptSchema(document), { defaultNonNullable: false })),
    'runtime.ts': await readFile(new URL('./templates/http-runtime.ts', import.meta.url), 'utf8'),
    ...await operationValidators(document),
  };
}

export { effectFiles } from './effect.mjs';
