import Ajv from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';
import standaloneCode from 'ajv/dist/standalone/index.js';
import { build } from 'esbuild';

export function responseSchemas(document) {
  const names = new Set(['MetadataOutput']);
  for (const route of Object.values(document.paths)) {
    if (!route.post) continue;
    for (const response of Object.values(route.post.responses)) {
      names.add(response.content['application/json'].schema.$ref.split('/').at(-1));
    }
  }
  return [...names].sort();
}

export async function validators(document, names = responseSchemas(document), prefix = 'validators', compatibility = true) {
  const schemas = structuredClone(document.components.schemas);
  // Compatibility is checked after validating the metadata's shape.
  if (compatibility) delete schemas.MetadataOutput.properties.protocol_version.const;
  const ajv = new Ajv({ code: { source: true, esm: true }, strict: true, allowUnionTypes: true });
  addFormats(ajv);
  for (const format of ['int64', 'int32', 'uint64', 'uint32', 'uint16', 'uint8', 'uint', 'int', 'float', 'double']) ajv.addFormat(format, true);
  ajv.addKeyword({ keyword: 'components', schemaType: 'object', valid: true });
  ajv.addKeyword({ keyword: 'x-provenance-model-family', schemaType: 'string', valid: true });
  const id = 'urn:provenance:operation-responses';
  ajv.addSchema({ $id: id, components: { schemas } });
  const exports = Object.fromEntries(names.map(name => [name, `${id}#/components/schemas/${name}`]));
  const code = standaloneCode(ajv, exports);
  const bundled = await build({ stdin: { contents: code, resolveDir: import.meta.dirname, sourcefile: 'validators.js' }, bundle: true, platform: 'browser', format: 'esm', write: false, minify: true, legalComments: 'none' });
  return {
    [`${prefix}.mjs`]: '// Generated from OpenAPI. Do not edit.\n' + bundled.outputFiles[0].text,
    [`${prefix}.d.mts`]: '// Generated from OpenAPI. Do not edit.\n' + names.map(name => `export declare function ${name}(value: unknown): boolean;`).join('\n') + '\n',
  };
}
