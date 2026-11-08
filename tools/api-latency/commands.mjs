// The porcelain commands and the MCP tool actions. The command and tool
// lists come from the binary under test. The tables below give only the
// arguments of the commands that need them; a command with no entry runs
// with no arguments, and a failure is reported with its reason.

/** Arguments for porcelain commands, by command path. `{name}` is a value from the run context. */
export const COMMAND_ARGUMENTS = {
  search: [['search', '--text', '{word}', '--format', 'json']],
  check: [
    ['check', '--format', 'json'],
    ['check', '--graph', '--format', 'json'],
    ['check', '--statements', '--format', 'json'],
    ['check', '--bindings', '--format', 'json'],
    ['check', '--gaps', '--format', 'json'],
    ['check', '--docs', '--format', 'json'],
    ['check', '--statement', '{statement}', '--format', 'json'],
  ],
  prime: [['prime', '--format', 'json']],
  init: [['init', '--path', '.']],
  materialize: [['materialize', '--format', 'json']],
  export: [['export', '--output', '{scratch}/export.json']],
  import: [['import', '--input', '{scratch}/export.json', '--dry-run', '--format', 'json']],
  // One row for each artifact kind of `validate --help`. The input is the first record of that kind.
  validate: [['validate', '{artifact}', '--input', '{scratch}/{artifact}.json']],
  'schema show': [['schema', 'show', '{artifact}']],
  'skills show': [['skills', 'show', '{skill}']],
  'report build': [['report', 'build', '--base', '{base}', '--head', 'HEAD', '--repository', 'quality-sh/provenance', '--output', '{scratch}/report.json']],
  'report render': [['report', 'render', '--input', '{scratch}/report.json']],
};

/** Commands that the benchmark does not call, with the reason. */
export const NOT_CALLED = {
  review: 'a long-running host: the HTTP rows time the calls that it serves',
  mcp: 'a long-running server: the MCP rows time the calls that it serves',
};

/** Commands that read a file that another command writes. Each runs after that command. */
const AFTER = { 'report render': 'report build', import: 'export' };

/** The command paths in help order, with each command of AFTER moved after the command it reads from. */
export function orderLeaves(leaves) {
  const ordered = leaves.filter(leaf => !AFTER[leaf]);
  for (const leaf of leaves.filter(item => AFTER[item])) {
    const at = ordered.indexOf(AFTER[leaf]);
    ordered.splice(at < 0 ? ordered.length : at + 1, 0, leaf);
  }
  return ordered;
}

/** The record route that each write verb of `provenance <id> <verb>` and of the MCP change tool uses. */
export const VERB_ROUTES = {
  create: { method: 'post', shape: '/{c}' },
  update: { method: 'patch', shape: '/{c}/{id}' },
  discuss: { method: 'post', shape: '/{c}/{id}/discussions' },
  reply: { method: 'post', shape: '/{c}/{id}/discussions/{discussion_id}/messages' },
};

/** The collections that a verb prefers, first match wins. */
const PREFERRED = ['/requirements', '/rules', '/sources', '/topics', '/questions', '/resolutions', '/boundaries'];

/** The read actions of `provenance <id> <action>`. They are not writes, whatever route has their name. */
export const READ_VERBS = ['get', 'trace', 'discussions', 'discussion'];

/** The write operation for a verb, or null. */
export function verbOperation(verb, operations) {
  if (READ_VERBS.includes(verb)) {
    return null;
  }
  const writes = operations.filter(op => op.write && !op.selector);
  const fixed = VERB_ROUTES[verb];
  const matches = writes.filter(op => {
    const parts = op.path.split('/').filter(Boolean);
    if (fixed) {
      const shape = fixed.shape.split('/').filter(Boolean);
      return op.method === fixed.method && parts.length === shape.length
        && shape.every((part, index) => part === '{c}' || part.startsWith('{') === parts[index].startsWith('{') && (part.startsWith('{') || part === parts[index]));
    }
    return op.method === 'post' && parts.length === 3 && parts[2] === verb;
  });
  const rank = op => {
    const index = PREFERRED.indexOf(`/${op.path.split('/')[1]}`);
    return index < 0 ? PREFERRED.length : index;
  };
  return matches.sort((a, b) => rank(a) - rank(b))[0] ?? null;
}

/** The argument lists of one command: one for each artifact kind where the arguments name `{artifact}`. */
export function commandVariants(variants, artifacts) {
  return variants.flatMap(args => (args.some(arg => arg.includes('{artifact}'))
    ? artifacts.map(artifact => args.map(arg => arg.replaceAll('{artifact}', artifact)))
    : [args]));
}

/** Replace each `{name}` in each argument with its context value. */
export function fillArguments(args, context) {
  return args.map(arg => arg.replace(/\{([a-z_]+)\}/g, (whole, name) => (context[name] === undefined ? whole : String(context[name]))));
}

function cliName(name) {
  return name.toLowerCase().replaceAll('_', '-');
}

/** The target and the field values of a planned record write. */
export function writeParts(verb, op, request) {
  const parts = request.path.split('/').filter(Boolean).map(decodeURIComponent);
  const body = { ...(request.body ?? {}) };
  let target;
  if (verb === 'create') {
    target = body.id;
    delete body.id;
  } else if (verb === 'reply') {
    target = parts[3];
  } else {
    target = parts[1];
  }
  return { target, kind: op.path.split('/')[1].replace(/ies$/, 'y').replace(/s$/, ''), body, ifMatch: request.headers?.['If-Match'] ?? null };
}

/** `provenance <target> <verb>` arguments for a planned record write. */
export function cliWriteArguments(verb, op, request) {
  const { target, kind, body, ifMatch } = writeParts(verb, op, request);
  const args = [target, verb, '--format', 'json'];
  if (verb === 'create') {
    args.push('--type', kind === 'boundarie' ? 'boundary' : kind);
  }
  if (ifMatch && verb === 'update') {
    args.push('--if-match', ifMatch);
  }
  if (ifMatch && verb === 'reply') {
    args.push('--expected-version', ifMatch);
  }
  for (const [name, value] of Object.entries(body)) {
    if (value === null || value === undefined) {
      continue;
    }
    if (typeof value === 'object') {
      args.push(`--${cliName(name)}-json`, JSON.stringify(value));
    } else {
      args.push(`--${cliName(name)}`, String(value));
    }
  }
  return args;
}

/** MCP `change` tool arguments for a planned record write. */
export function mcpWriteArguments(verb, op, request) {
  const { target, kind, body, ifMatch } = writeParts(verb, op, request);
  const args = { action: verb, target, ...body };
  if (verb === 'create') {
    args.type = kind === 'boundarie' ? 'boundary' : kind;
  }
  if (ifMatch && verb === 'update') {
    args.if_match = ifMatch;
  }
  if (ifMatch && verb === 'reply') {
    args.expected_version = Number(ifMatch);
  }
  return args;
}
