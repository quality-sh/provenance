// Turns the catalog that `provenance api` lists into the operations that the
// benchmark times. The catalog comes from the binary under test, so a new
// route or query is timed without a change here.

/** The selector of a query variant: the const value of its `query` parameter. */
export function selectorOf(variant) {
  const parameter = (variant.parameters ?? []).find(item => item.name === 'query' && item.location === 'query');
  return parameter?.schema?.const ?? null;
}

/** The label of one operation, for example `GET /rules/{id} query=trace`. */
export function operationLabel(method, path, selector) {
  const head = `${method.toUpperCase()} ${path}`;
  return selector ? `${head} query=${selector}` : head;
}

/**
 * One operation for each query variant of each route. A GET is a read.
 * A POST or PATCH is a write, except where the description says that the
 * route only checks or plans, which the `readOnlyPosts` list names by path.
 */
export function operationsFromCatalog(catalog, { readOnlyPosts = [] } = {}) {
  if (catalog?.bounds?.truncated) {
    throw new Error(`the catalog listing is truncated at ${catalog.bounds.returned} of ${catalog.bounds.total} routes`);
  }
  const operations = [];
  for (const route of catalog.routes ?? []) {
    const variants = route.queries?.length ? route.queries : [{ parameters: [] }];
    for (const variant of variants) {
      const selector = selectorOf(variant);
      const method = route.method.toLowerCase();
      operations.push({
        label: operationLabel(method, route.path, selector),
        method,
        path: route.path,
        selector,
        parameters: variant.parameters ?? [],
        requestSchema: route.request_schema ?? null,
        write: method !== 'get' && !readOnlyPosts.includes(route.path),
      });
    }
  }
  return operations;
}

/** The names of the path parameters in a route path, in order. */
export function pathParameters(path) {
  return [...path.matchAll(/\{([^}]+)\}/g)].map(match => match[1]);
}

/** Replace each `{name}` in a route path with its encoded value. */
export function fillPath(path, values) {
  return path.replace(/\{([^}]+)\}/g, (_, name) => {
    if (values[name] === undefined) {
      throw new Error(`no value for path parameter ${name}`);
    }
    return encodeURIComponent(values[name]);
  });
}

/** The subcommand names in the `Commands:` section of clap help text. */
export function helpCommands(text) {
  const lines = text.split('\n');
  const start = lines.findIndex(line => line.trim() === 'Commands:');
  if (start < 0) {
    return [];
  }
  const names = [];
  for (const line of lines.slice(start + 1)) {
    if (!line.startsWith('  ')) {
      break;
    }
    const name = line.trim().split(/\s+/)[0];
    if (name && name !== 'help') {
      names.push(name);
    }
  }
  return names;
}

/** The values of `[possible values: ...]` for one argument in clap help text. */
export function possibleValues(text, argument) {
  const lines = text.split('\n');
  const start = lines.findIndex(item => item.trimStart().startsWith(argument));
  if (start < 0) {
    return [];
  }
  // The values are on the argument line, or on a later line before the next argument.
  for (const line of lines.slice(start, start + 6)) {
    if (line !== lines[start] && /^(-|\[[A-Z]|<)/.test(line.trimStart())) {
      break;
    }
    const match = line.match(/\[possible values: ([^\]]+)\]/);
    if (match) {
      return match[1].split(',').map(item => item.trim());
    }
  }
  return [];
}

/** One entry for each tool, or one for each action of a tool with an `action` enum. */
export function mcpToolActions(tools) {
  const calls = [];
  for (const tool of tools) {
    const actions = tool.inputSchema?.properties?.action?.enum;
    const readOnly = tool.annotations?.readOnlyHint === true;
    if (actions?.length) {
      for (const action of actions) {
        calls.push({ label: `tool ${tool.name} action=${action}`, tool: tool.name, action, readOnly });
      }
    } else {
      calls.push({ label: `tool ${tool.name}`, tool: tool.name, action: null, readOnly });
    }
  }
  return calls;
}
