/** A failed refresh removes the previous view. Only the latest request can mount. */
export function createSession<T extends { dispose(): void }>(view: { mount(value: T): () => void; status(message: string): void; failure?(error: unknown): string | undefined }) {
  let generation = 0;
  let unmount: (() => void) | undefined;
  return {
    async refresh(load: () => Promise<T>) {
      const request = ++generation;
      unmount?.();
      unmount = undefined;
      view.status('Loading the saved document…');
      try {
        const value = await load();
        if (request !== generation) { value.dispose(); return; }
        unmount = view.mount(value);
        view.status('Read-only · Document status is shown below.');
      } catch (error) {
        if (request !== generation) return;
        view.status(view.failure?.(error) ?? 'A current complete document is unavailable. Check access, the Requirement ID, and saved graph validity, then refresh. The Requirement can be retired, or the response can exceed the client size limit.');
      }
    },
  };
}
