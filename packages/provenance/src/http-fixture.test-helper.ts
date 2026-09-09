import { after } from 'node:test';
import { startFixtureHost } from '../scripts/fixture-host.js';
import type { ConfigureOptions } from './settings.js';

const running = new Map<string, Promise<ConfigureOptions>>();
const closing: Array<() => Promise<void>> = [];
after(async () => { await Promise.all(closing.map(close => close())); });

export function fixtureSettings(root: string): Promise<ConfigureOptions> {
  let settings = running.get(root);
  if (!settings) {
    settings = startFixtureHost({ root }).then(host => {
      closing.push(() => host.close());
      return {
        endpoint: host.environment.PROVENANCE_ENDPOINT,
        bearer: host.environment.PROVENANCE_TOKEN,
        repositoryId: host.environment.PROVENANCE_REPOSITORY_ID,
        localRoot: root,
        scope: host.environment.PROVENANCE_SCOPE,
      };
    });
    running.set(root, settings);
  }
  return settings;
}
