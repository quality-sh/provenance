export interface ReviewConfig {
  readonly endpoint: string;
  readonly repositoryId: string;
  readonly scope: string;
  readonly dispositionActorIds: ReadonlyArray<string>;
}

export interface ReviewMountOptions extends ReviewConfig {
  readonly bearer: string;
  readonly rootId: string;
  readonly focusId?: string;
}

/** Keeps the credential in memory and lets only the latest connection take effect. */
export function createSession(view: {
  mount(options: ReviewMountOptions): () => void;
  connected(config: ReviewConfig): void;
  status(message: string): void;
}) {
  let generation = 0;
  let connection: (ReviewConfig & { bearer: string }) | undefined;
  let unmount: (() => void) | undefined;

  return {
    async connect(bearer: string, load: () => Promise<ReviewConfig>) {
      const request = ++generation;
      connection = undefined;
      unmount?.();
      unmount = undefined;
      view.status('Connecting…');
      try {
        const config = await load();
        if (request !== generation) return;
        connection = { ...config, bearer };
        view.connected(config);
        view.status(`Connected · ${config.repositoryId} / ${config.scope}`);
      } catch {
        if (request !== generation) return;
        view.status('Connection refused. Enter the access token from this host session.');
      }
    },
    open(rootId: string, focusId?: string) {
      if (!connection) return false;
      unmount?.();
      unmount = view.mount({ ...connection, rootId, ...(focusId === undefined ? {} : { focusId }) });
      view.status(`Reviewing ${rootId} · ${connection.repositoryId} / ${connection.scope}`);
      return true;
    },
  };
}
