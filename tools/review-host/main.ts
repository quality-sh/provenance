import { mountReview } from 'review-renderer';
import { bootstrapReviewPage, EVENT_PROTOCOL, type BrowserElement } from './bootstrap.ts';
import type { EventSocket } from './events.ts';

bootstrapReviewPage({
  document: document as unknown as { getElementById(id: string): BrowserElement | null },
  location,
  history,
  fetch,
  mount: (root, options) => mountReview(root as unknown as Element, options),
  storage: localStorage,
  openEvents: session => {
    const socket = new WebSocket(`ws://${location.host}/review-events`, [EVENT_PROTOCOL, session]);
    const events: EventSocket = {
      onmessage: null, onclose: null,
      send: text => socket.send(text),
      close: () => socket.close(),
    };
    socket.onmessage = event => events.onmessage?.(event);
    socket.onclose = () => events.onclose?.();
    return events;
  },
  onWake: listener => {
    window.addEventListener('online', listener);
    window.addEventListener('focus', listener);
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') listener();
    });
  },
});
