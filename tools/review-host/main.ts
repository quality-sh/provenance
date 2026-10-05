import { mountReview } from 'review-renderer';
import { bootstrapReviewPage, type BrowserElement } from './bootstrap.ts';

bootstrapReviewPage({
  document: document as unknown as { getElementById(id: string): BrowserElement | null },
  location,
  history,
  fetch,
  mount: (root, options) => mountReview(root as unknown as Element, options),
});
