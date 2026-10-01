import { test, expect } from '@playwright/test';

/**
 * The world outline is bundled from the world-atlas package: the map must load
 * it from the app's own origin and show an error when that chunk fails.
 */
test.describe('World atlas', () => {
  test('loads the outline without an external request', async ({ page, baseURL }) => {
    const appOrigin = new URL(baseURL ?? 'http://localhost:3000').origin;
    const externalRequests: string[] = [];
    page.on('request', request => {
      const url = new URL(request.url());
      const external = url.origin !== appOrigin;
      if (
        url.hostname === 'cdn.jsdelivr.net' ||
        (external && /world-atlas|countries-\d+m/.test(url.pathname))
      ) {
        externalRequests.push(url.href);
      }
    });

    await page.goto('/global');

    await expect(page.locator('svg path.country').first()).toBeAttached();
    expect(externalRequests).toEqual([]);
  });

  test('shows an error with a reload action when the outline fails to load', async ({ page }) => {
    await page.route('**/*countries-50m*', route => route.abort());

    await page.goto('/global');

    await expect(page.getByText('Failed to load world map data')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Reload' })).toBeVisible();
  });
});
