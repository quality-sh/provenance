import type { TypedSpecDocument } from './protocol.js';
import { portableFile } from './portable-file.js';

export function documentPaths(document: TypedSpecDocument, localRoot?: string) {
  return { ...document, rules: document.rules.map(rule => ({
    ...rule,
    address: rule.address === undefined ? undefined : [...rule.address],
    implementation: rule.implementation === undefined ? undefined : {
      ...rule.implementation, file: portableFile(rule.implementation.file, localRoot),
    },
  })) };
}
