/**
 * Demo Playwright Test for Crawler Admin Interface
 *
 * This test demonstrates the key features of the crawler admin interface
 * and captures screenshots/videos for demonstration purposes.
 */

import { test, expect } from "@playwright/test";

test.describe("Crawler Admin Demo", () => {
  test("should demonstrate the complete crawler admin interface", async ({
    page,
  }) => {
    // Navigate to the admin interface
    await page.goto("http://localhost:3001");

    // Wait for the main app to load
    await expect(page.locator('[data-testid="dashboard-title"]')).toContainText(
      "Crawler Administration",
    );

    // Take screenshot of the main dashboard
    await page.screenshot({
      path: "test-results/dashboard-overview.png",
      fullPage: true,
    });

    // Test Dashboard Tab
    await page.locator('[role="tab"]').nth(0).click();
    await expect(
      page.locator('[data-testid="crawler-status-card"]'),
    ).toBeVisible();
    await expect(
      page.locator('[data-testid="queue-statistics-card"]'),
    ).toBeVisible();
    await expect(
      page.locator('[data-testid="crawler-status-title"]'),
    ).toBeVisible();
    await expect(
      page.locator('[data-testid="queue-statistics-title"]'),
    ).toBeVisible();

    // Take screenshot of dashboard content
    await page.screenshot({
      path: "test-results/dashboard-content.png",
      fullPage: true,
    });

    // Test Configuration Tab
    await page.locator('[role="tab"]').nth(1).click();
    await expect(page.locator('[data-testid="config-title"]')).toBeVisible();
    await expect(
      page.locator('[data-testid="global-settings-title"]'),
    ).toBeVisible();
    await expect(
      page.locator('[data-testid="data-sources-title"]'),
    ).toBeVisible();

    // Take screenshot of configuration page
    await page.screenshot({
      path: "test-results/configuration-page.png",
      fullPage: true,
    });

    // Test Logs & Monitoring Tab
    await page.locator('[role="tab"]').nth(2).click();
    await expect(page.locator('[data-testid="logs-title"]')).toBeVisible();

    // Take screenshot of logs page
    await page.screenshot({
      path: "test-results/logs-page.png",
      fullPage: true,
    });

    // Go back to dashboard and test controls
    await page.locator('[role="tab"]').nth(0).click();

    // Test refresh button
    const refreshButton = page.locator('[data-testid="refresh-button"]');
    await expect(refreshButton).toBeVisible();
    await refreshButton.click();

    // Test trigger crawl button
    const triggerButton = page.locator('[data-testid="trigger-crawl-button"]');
    await expect(triggerButton).toBeVisible();

    // Test stop crawler button
    const stopButton = page.locator('[data-testid="stop-crawler-button"]');
    await expect(stopButton).toBeVisible();

    // Take final screenshot showing all controls
    await page.screenshot({
      path: "test-results/controls-demo.png",
      fullPage: true,
    });
  });

  test("should demonstrate mobile responsiveness", async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });

    await page.goto("http://localhost:3001");

    // Take mobile screenshot
    await page.screenshot({
      path: "test-results/mobile-view.png",
      fullPage: true,
    });

    // Test mobile navigation
    await page.locator('[role="tab"]').nth(1).click();
    await page.screenshot({
      path: "test-results/mobile-config.png",
      fullPage: true,
    });
  });

  test("should demonstrate tablet responsiveness", async ({ page }) => {
    // Set tablet viewport
    await page.setViewportSize({ width: 768, height: 1024 });

    await page.goto("http://localhost:3001");

    // Take tablet screenshot
    await page.screenshot({
      path: "test-results/tablet-view.png",
      fullPage: true,
    });
  });
});
