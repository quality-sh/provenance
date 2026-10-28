import { isStringEnum, operations, pascal, propertyName, queryVariants } from './shared.mjs';

const keywordVariants = new Set(['Self', 'Super', 'Crate']);

export const enumVariant = value => {
  const variant = pascal(value);
  return keywordVariants.has(variant) ? `${variant}Value` : variant;
};

export const parameterEnumKey = (schema, parameter) => JSON.stringify([propertyName(parameter), schema.enum]);

/// One closed enum per declared parameter value set, shared by every method
/// that publishes the parameter. Names start from the parameter's PascalCase
/// name; a name already taken by a different value set extends with its
/// values, then a deterministic suffix.
export function allocateEnums(document) {
  const allocated = new Map();
  const used = new Set();
  for (const { op } of operations(document).filter(({ op }) => op.operationId !== 'metadata')) {
    const variants = queryVariants(op);
    const parameters = variants.length
      ? variants.flatMap(variant => variant.parameters)
      : (op.parameters ?? []);
    for (const parameter of parameters) {
      const schemas = [parameter.schema];
      if (parameter.schema?.type === 'array') schemas.push(parameter.schema.items);
      for (const schema of schemas) {
        if (!isStringEnum(schema)) continue;
        const key = parameterEnumKey(schema, parameter);
        if (allocated.has(key)) continue;
        const values = schema.enum;
        const base = pascal(propertyName(parameter));
        let name = base;
        if (used.has(name)) name = `${base}${values.map(enumVariant).join('')}`;
        let suffix = 2;
        while (used.has(name)) name = `${base}Variant${suffix++}`;
        used.add(name);
        allocated.set(key, name);
      }
    }
  }
  return allocated;
}

/// The shared module of closed parameter enums. `as_str` returns the exact
/// wire token, so valid values keep their encoding.
export function parametersModule(enums) {
  const byName = new Map();
  for (const [key, name] of enums) byName.set(name, JSON.parse(key)[1]);
  const types = [...byName.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([name, values]) => `#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ${name} {
${values.map(value => `    ${enumVariant(value)},`).join('\n')}
}
impl ${name} {
    pub const fn as_str(self) -> &'static str {
        match self {
${values.map(value => `            Self::${enumVariant(value)} => ${JSON.stringify(value)},`).join('\n')}
        }
    }
}`);
  return `// Generated from OpenAPI. Do not edit.
// Closed enums for every declared operation parameter value set.
${types.join('\n')}
`;
}
