import { mountReview } from 'review-renderer';
import { bootstrapReviewPage, type BrowserElement } from './bootstrap.ts';

bootstrapReviewPage({
  document: document as unknown as { getElementById(id: string): BrowserElement | null },
  location,
  history,
  fetch,
  mount: (root, options) => mountReview(root as unknown as Element, options),
  openEvents: () => new WebSocket(`ws://${location.host}/review-events`),
  onWake: listener => {
    window.addEventListener('online', listener);
    window.addEventListener('focus', listener);
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') listener();
    });
  },
});
