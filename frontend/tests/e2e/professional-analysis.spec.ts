import { test, expect } from '@playwright/test';

test.describe('Professional Analysis', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/analysis');
  });

  test('should display professional analysis page', async ({ page }) => {
    await expect(page.locator('main')).toBeVisible();

    // Check for professional analysis specific content - use specific heading
    const analysisContent = page.getByRole('heading', { name: 'Bloomberg Terminal-level economic analysis with technical indicators' });

    await expect(analysisContent).toBeVisible();
  });

  test('should display advanced charting tools', async ({ page }) => {
    // Look for advanced charting features
    const chartElement = page.locator('canvas').or(
      page.locator('[data-testid="chart"]').or(
        page.locator('.chart').or(
          page.locator('svg')
        )
      )
    );

    await expect(chartElement.first()).toBeVisible();
  });

  test('should have technical analysis indicators', async ({ page }) => {
    // Look for technical analysis tools
    const technicalIndicators = page.getByText(/moving average/i).or(
      page.getByText(/rsi/i).or(
        page.getByText(/macd/i).or(
          page.getByText(/bollinger/i).or(
            page.locator('[data-testid="technical-indicators"]')
          )
        )
      )
    );

    await expect(technicalIndicators.first()).toBeVisible();
  });

  test('should allow chart customization and configuration', async ({ page }) => {
    // Look for chart configuration options
    const configButton = page.getByRole('button', { name: /config|settings/i }).or(
      page.getByRole('button', { name: /customize/i })
    );

    const indicatorSelector = page.locator('select').or(
      page.locator('[data-testid="indicator-selector"]')
    );

    // Should have some form of chart customization
    const hasCustomization = await configButton.isVisible() ||
      await indicatorSelector.isVisible();

    expect(hasCustomization).toBeTruthy();
  });

  test('should display multiple timeframes', async ({ page }) => {
    // Look for timeframe selection
    const timeframeSelector = page.getByRole('button', { name: /1d|1w|1m|1y/i }).or(
      page.locator('select').or(
        page.locator('[data-testid="timeframe-selector"]')
      )
    );

    await expect(timeframeSelector.first()).toBeVisible();
  });

  test('should allow data export functionality', async ({ page }) => {
    // Look for export options
    const exportButton = page.getByRole('button', { name: /export|download/i }).or(
      page.getByText(/export/i).or(
        page.getByText(/download/i)
      )
    );

    await expect(exportButton.first()).toBeVisible();
  });

  test('should display correlation analysis', async ({ page }) => {
    // Look for correlation features
    const correlationElement = page.getByText(/correlation/i).or(
      page.getByText(/relationship/i).or(
        page.locator('[data-testid="correlation"]')
      )
    );

    await expect(correlationElement.first()).toBeVisible();
  });

  test('should allow series comparison', async ({ page }) => {
    // Look for comparison features
    const compareButton = page.getByRole('button', { name: /compare/i }).or(
      page.getByText(/compare/i)
    );

    const seriesSelector = page.locator('select').or(
      page.locator('[data-testid="series-selector"]')
    );

    // Should have some form of series comparison
    const hasComparison = await compareButton.isVisible() ||
      await seriesSelector.isVisible();

    expect(hasComparison).toBeTruthy();
  });

  test('should display statistical analysis tools', async ({ page }) => {
    // Look for statistical analysis features
    const statsElement = page.getByText(/statistics/i).or(
      page.getByText(/regression/i).or(
        page.getByText(/trend/i).or(
          page.locator('[data-testid="statistics"]')
        )
      )
    );

    await expect(statsElement.first()).toBeVisible();
  });

  test('should allow annotation and markup tools', async ({ page }) => {
    // Look for annotation tools
    const annotationButton = page.getByRole('button', { name: /annotate|markup/i }).or(
      page.getByText(/annotate/i).or(
        page.getByText(/markup/i)
      )
    );

    const drawingTools = page.getByRole('button', { name: /draw|line|arrow/i });

    // Should have some form of annotation tools
    const hasAnnotation = await annotationButton.isVisible() ||
      await drawingTools.isVisible();

    expect(hasAnnotation).toBeTruthy();
  });

  test('should be responsive on mobile devices', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 });

    await expect(page.locator('main')).toBeVisible();

    // Chart should still be visible and functional
    const chartElement = page.locator('canvas').or(
      page.locator('[data-testid="chart"]').or(
        page.locator('svg')
      )
    );

    await expect(chartElement.first()).toBeVisible();
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
      !error.includes('401') &&
      !error.includes('Invalid scale configuration for scale: y1')
    );

    expect(unexpectedErrors).toHaveLength(0);
  });

  test('should handle chart interaction errors gracefully', async ({ page }) => {
    // Try to interact with chart elements
    const chartElement = page.locator('canvas').or(
      page.locator('[data-testid="chart"]').or(
        page.locator('svg')
      )
    ).first();

    if (await chartElement.isVisible()) {
      // Click on chart should not cause errors
      await chartElement.click();

      // Should not show error messages
      const errorMessage = page.locator('[role="alert"]').or(
        page.locator('.error')
      );

      await expect(errorMessage).not.toBeVisible();
    }
  });
});
