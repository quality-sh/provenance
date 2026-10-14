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

/** A record that a saved change added, changed or deleted. */
export interface ChangedRecord {
  readonly kind: string;
  readonly id: string;
}

/** The renderer reasons for reading all saved state again. */
export type RereadReason =
  | 'ready'
  | 'sequence-gap'
  | 'file-events-lost'
  | 'observation-recovered'
  | 'subscriber-loss';

/** What the page reads again after a saved change. */
export type ReviewChange =
  | { readonly _tag: 'Everything'; readonly reason: RereadReason }
  | { readonly _tag: 'Records'; readonly records: ReadonlyArray<ChangedRecord> };

/** The mounted renderer. */
export interface ReviewHandle {
  dispose(): void;
  refresh(change: ReviewChange): void;
  showRecord(rootId: string, focusId?: string): Promise<void>;
}

/** Tells the person why the page did not sign in and how to get a new review link. */
export const CONNECTION_REFUSED =
  'Connection refused. This review link was used, expired, or is for a different review host. ' +
  'To get a new link, run `provenance <record-id> --review-link`.';

/** Keeps the credential in memory and lets only the latest connection take effect. */
export function createSession(view: {
  mount(options: ReviewMountOptions): ReviewHandle;
  connected(config: ReviewConfig): void;
  status(message: string): void;
}) {
  let generation = 0;
  let connection: (ReviewConfig & { bearer: string }) | undefined;
  let page: ReviewHandle | undefined;
  let reviewing = '';

  return {
    async connect(bearer: string, load: () => Promise<ReviewConfig>) {
      const request = ++generation;
      connection = undefined;
      page?.dispose();
      page = undefined;
      view.status('Connecting…');
      try {
        const config = await load();
        if (request !== generation) return;
        connection = { ...config, bearer };
        view.connected(config);
        view.status(`Connected · ${config.repositoryId} / ${config.scope}`);
      } catch {
        if (request !== generation) return;
        view.status(CONNECTION_REFUSED);
      }
    },
    open(rootId: string, focusId?: string) {
      if (!connection) return false;
      page?.dispose();
      page = view.mount({ ...connection, rootId, ...(focusId === undefined ? {} : { focusId }) });
      reviewing = `Reviewing ${rootId} · ${connection.repositoryId} / ${connection.scope}`;
      view.status(reviewing);
      return true;
    },
    /** Shows a review link in the mounted page, so drafts stay; without a page it mounts one. */
    async show(rootId: string, focusId?: string) {
      if (page) {
        await page.showRecord(rootId, focusId);
        if (connection) reviewing = `Reviewing ${rootId} · ${connection.repositoryId} / ${connection.scope}`;
        view.status(reviewing);
        return;
      }
      this.open(rootId, focusId);
    },
    refresh(change: ReviewChange) {
      page?.refresh(change);
    },
    /** The status line of the current review, after a live update notice. */
    restoreStatus() {
      if (reviewing !== '') view.status(reviewing);
    },
  };
}
