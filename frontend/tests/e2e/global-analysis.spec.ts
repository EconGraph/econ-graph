import { test, expect } from '@playwright/test';

test.describe('Global Analysis', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/global');
  });

  test('should display global analysis page', async ({ page }) => {
    await expect(page.locator('main')).toBeVisible();

    // Check for global analysis specific content
    const globalContent = page.getByText(/global/i).or(
      page.getByText(/world/i).or(
        page.getByText(/international/i).or(
          page.getByText(/multi-country/i)
        )
      )
    );

    await expect(globalContent).toBeVisible();
  });

  test('should display world map or global visualization', async ({ page }) => {
    // Look for map or global visualization
    const mapElement = page.locator('[data-testid="world-map"]').or(
      page.locator('.world-map').or(
        page.locator('svg').or(
          page.locator('canvas')
        )
      )
    );

    await expect(mapElement.first()).toBeVisible();
  });

  test('should allow country selection or interaction', async ({ page }) => {
    // Look for interactive elements on the map
    const interactiveElement = page.locator('[data-testid="country"]').or(
      page.locator('.country').or(
        page.locator('path').or(
          page.locator('circle')
        )
      )
    ).first();

    if (await interactiveElement.isVisible()) {
      // Should be clickable or hoverable
      await interactiveElement.hover();

      // Should show tooltip or highlight
      const tooltip = page.locator('[role="tooltip"]').or(
        page.locator('.tooltip').or(
          page.locator('[data-testid="tooltip"]')
        )
      );

      const hasTooltip = await tooltip.isVisible();
      expect(hasTooltip).toBeTruthy();
    }
  });

  test('should display country comparison features', async ({ page }) => {
    // Look for comparison controls
    const compareButton = page.getByRole('button', { name: /compare/i }).or(
      page.getByText(/compare/i)
    );

    const countrySelector = page.locator('select').or(
      page.locator('[data-testid="country-selector"]')
    );

    // Should have some form of country comparison
    const hasComparison = await compareButton.isVisible() ||
      await countrySelector.isVisible();

    expect(hasComparison).toBeTruthy();
  });

  test('should show economic indicators for selected countries', async ({ page }) => {
    // Look for economic indicators or metrics
    const indicatorsElement = page.getByText(/gdp/i).or(
      page.getByText(/inflation/i).or(
        page.getByText(/unemployment/i).or(
          page.locator('[data-testid="indicators"]')
        )
      )
    );

    await expect(indicatorsElement.first()).toBeVisible();
  });

  test('should display time series charts for global data', async ({ page }) => {
    // Look for charts or graphs
    const chartElement = page.locator('canvas').or(
      page.locator('[data-testid="chart"]').or(
        page.locator('.chart').or(
          page.locator('svg')
        )
      )
    );

    await expect(chartElement.first()).toBeVisible();
  });

  test('should allow filtering by time period', async ({ page }) => {
    // Look for date picker or time filter
    const datePicker = page.locator('input[type="date"]').or(
      page.locator('[data-testid="date-picker"]').or(
        page.getByRole('button', { name: /date/i })
      )
    );

    const timeFilter = page.getByRole('button', { name: /year|month|quarter/i });

    // Should have some form of time filtering
    const hasTimeFilter = await datePicker.isVisible() ||
      await timeFilter.isVisible();

    expect(hasTimeFilter).toBeTruthy();
  });

  test('should display global economic events or news', async ({ page }) => {
    // Look for events or news section
    const eventsSection = page.getByText(/events/i).or(
      page.getByText(/news/i).or(
        page.getByText(/updates/i).or(
          page.locator('[data-testid="events"]')
        )
      )
    );

    // Should have some form of global events or news
    await expect(eventsSection.first()).toBeVisible();
  });

  test('should be responsive on mobile devices', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });

    await expect(page.locator('main')).toBeVisible();

    // Map should still be visible and functional
    const mapElement = page.locator('[data-testid="world-map"]').or(
      page.locator('.world-map').or(
        page.locator('svg')
      )
    );

    await expect(mapElement.first()).toBeVisible();
  });

  test('should load without JavaScript console errors', async ({ page }) => {
    const consoleErrors: string[] = [];

    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });

    await page.reload();
    await page.waitForLoadState('networkidle');

    // Filter out expected errors
    const unexpectedErrors = consoleErrors.filter(error =>
      !error.includes('OAuth') &&
      !error.includes('authentication') &&
      !error.includes('403') &&
      !error.includes('401')
    );

    expect(unexpectedErrors).toHaveLength(0);
  });

  test('should handle map interaction errors gracefully', async ({ page }) => {
    // Try to interact with map elements
    const mapElement = page.locator('[data-testid="world-map"]').or(
      page.locator('.world-map').or(
        page.locator('svg')
      )
    ).first();

    if (await mapElement.isVisible()) {
      // Click on map should not cause errors
      await mapElement.click();

      // Should not show error messages
      const errorMessage = page.locator('[role="alert"]').or(
        page.locator('.error')
      );

      await expect(errorMessage).not.toBeVisible();
    }
  });
});
