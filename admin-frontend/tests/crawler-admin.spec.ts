/**
 * Playwright E2E Tests for Crawler Admin Interface
 *
 * Tests the complete crawler administration workflow:
 * - Dashboard navigation and data display
 * - Real-time status monitoring
 * - Crawler control operations
 * - Configuration management
 * - Logs and monitoring
 */

import { test, expect } from '@playwright/test';

// Test configuration
test.describe('Crawler Admin Interface', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the admin interface
    await page.goto('http://localhost:3001');

    // Wait for the main app to load
    await expect(page.locator('h1')).toContainText('EconGraph Crawler Administration');
  });

  test.describe('Dashboard Navigation', () => {
    test('should display main navigation tabs', async ({ page }) => {
      // Check that all main tabs are present
      await expect(page.locator('[role="tab"]').nth(0)).toContainText('Dashboard');
      await expect(page.locator('[role="tab"]').nth(1)).toContainText('Configuration');
      await expect(page.locator('[role="tab"]').nth(2)).toContainText('Logs & Monitoring');
    });

    test('should switch between tabs correctly', async ({ page }) => {
      // Test Dashboard tab
      await page.locator('[role="tab"]').nth(0).click();
      await expect(page.locator('h1')).toContainText('🕷️ Crawler Administration');

      // Test Configuration tab
      await page.locator('[role="tab"]').nth(1).click();
      await expect(page.locator('h1')).toContainText('Crawler Configuration');

      // Test Logs & Monitoring tab
      await page.locator('[role="tab"]').nth(2).click();
      await expect(page.locator('h1')).toContainText('Logs & Monitoring');
    });
  });

  test.describe('Crawler Status Dashboard', () => {
    test('should display crawler status information', async ({ page }) => {
      // Ensure we're on the Dashboard tab
      await page.locator('[role="tab"]').nth(0).click();

      // Check crawler status card
      await expect(page.locator('text=Crawler Status')).toBeVisible();

      // Check status chip (Running/Stopped)
      const statusChip = page.locator('[data-testid="crawler-status-chip"]').or(
        page.locator('text=Running').or(page.locator('text=Stopped'))
      );
      await expect(statusChip).toBeVisible();

      // Check active workers display
      await expect(page.locator('text=Active Workers')).toBeVisible();

      // Check last crawl time
      await expect(page.locator('text=Last Crawl')).toBeVisible();

      // Check next scheduled crawl
      await expect(page.locator('text=Next Scheduled Crawl')).toBeVisible();
    });

    test('should display queue statistics', async ({ page }) => {
      // Ensure we're on the Dashboard tab
      await page.locator('[role="tab"]').nth(0).click();

      // Check queue statistics card
      await expect(page.locator('text=Queue Statistics')).toBeVisible();

      // Check all queue metrics
      await expect(page.locator('text=Total Items')).toBeVisible();
      await expect(page.locator('text=Pending')).toBeVisible();
      await expect(page.locator('text=Processing')).toBeVisible();
      await expect(page.locator('text=Completed')).toBeVisible();
      await expect(page.locator('text=Failed')).toBeVisible();
      await expect(page.locator('text=Retrying')).toBeVisible();

      // Check progress bar
      const progressBar = page.locator('[role="progressbar"]');
      await expect(progressBar).toBeVisible();
    });

    test('should display recent activity', async ({ page }) => {
      // Ensure we're on the Dashboard tab
      await page.locator('[role="tab"]').nth(0).click();

      // Check recent activity section
      await expect(page.locator('text=Recent Activity')).toBeVisible();

      // Check activity table headers
      await expect(page.locator('text=Timestamp')).toBeVisible();
      await expect(page.locator('text=Source')).toBeVisible();
      await expect(page.locator('text=Series')).toBeVisible();
      await expect(page.locator('text=Status')).toBeVisible();
      await expect(page.locator('text=Duration')).toBeVisible();
    });
  });

  test.describe('Crawler Controls', () => {
    test('should have refresh button functionality', async ({ page }) => {
      // Ensure we're on the Dashboard tab
      await page.locator('[role="tab"]').nth(0).click();

      // Find and click refresh button
      const refreshButton = page.locator('button:has-text("Refresh")');
      await expect(refreshButton).toBeVisible();

      // Click refresh and check for loading state
      await refreshButton.click();

      // Should show "Refreshing..." text briefly
      await expect(page.locator('text=Refreshing...')).toBeVisible({ timeout: 1000 });
    });

    test('should have trigger crawl button', async ({ page }) => {
      // Ensure we're on the Dashboard tab
      await page.locator('[role="tab"]').nth(0).click();

      // Find trigger crawl button
      const triggerButton = page.locator('button:has-text("Trigger Crawl")');
      await expect(triggerButton).toBeVisible();

      // Button should be clickable
      await expect(triggerButton).toBeEnabled();
    });

    test('should have stop crawler button', async ({ page }) => {
      // Ensure we're on the Dashboard tab
      await page.locator('[role="tab"]').nth(0).click();

      // Find stop crawler button
      const stopButton = page.locator('button:has-text("Stop Crawler")');
      await expect(stopButton).toBeVisible();

      // Button should be clickable
      await expect(stopButton).toBeEnabled();
    });
  });

  test.describe('Configuration Management', () => {
    test('should display configuration form', async ({ page }) => {
      // Navigate to Configuration tab
      await page.locator('[role="tab"]').nth(1).click();

      // Check configuration sections
      await expect(page.locator('text=Global Crawler Settings')).toBeVisible();
      await expect(page.locator('text=Data Sources')).toBeVisible();

      // Check form fields
      await expect(page.locator('text=Max Workers')).toBeVisible();
      await expect(page.locator('text=Queue Size Limit')).toBeVisible();
      await expect(page.locator('text=Default Timeout')).toBeVisible();
      await expect(page.locator('text=Retry Attempts')).toBeVisible();
    });

    test('should allow configuration editing', async ({ page }) => {
      // Navigate to Configuration tab
      await page.locator('[role="tab"]').nth(1).click();

      // Find and interact with max workers field
      const maxWorkersField = page.locator('input[name="max_workers"]').or(
        page.locator('input[placeholder*="workers" i]')
      );

      if (await maxWorkersField.isVisible()) {
        await maxWorkersField.clear();
        await maxWorkersField.fill('10');
      }

      // Check for save button
      const saveButton = page.locator('button:has-text("Save")');
      await expect(saveButton).toBeVisible();
    });

    test('should display data sources table', async ({ page }) => {
      // Navigate to Configuration tab
      await page.locator('[role="tab"]').nth(1).click();

      // Check data sources section
      await expect(page.locator('text=Data Sources')).toBeVisible();

      // Check for add data source button
      const addButton = page.locator('button:has-text("Add Data Source")').or(
        page.locator('button:has-text("New Data Source")')
      );
      await expect(addButton).toBeVisible();
    });
  });

  test.describe('Logs and Monitoring', () => {
    test('should display logs interface', async ({ page }) => {
      // Navigate to Logs & Monitoring tab
      await page.locator('[role="tab"]').nth(2).click();

      // Check logs section
      await expect(page.locator('text=System Logs')).toBeVisible();

      // Check log level filters
      await expect(page.locator('text=All Levels')).toBeVisible();
      await expect(page.locator('text=Error')).toBeVisible();
      await expect(page.locator('text=Warning')).toBeVisible();
      await expect(page.locator('text=Info')).toBeVisible();
    });

    test('should display performance metrics', async ({ page }) => {
      // Navigate to Logs & Monitoring tab
      await page.locator('[role="tab"]').nth(2).click();

      // Check performance metrics section
      await expect(page.locator('text=Performance Metrics')).toBeVisible();

      // Check metric cards
      await expect(page.locator('text=CPU Usage')).toBeVisible();
      await expect(page.locator('text=Memory Usage')).toBeVisible();
      await expect(page.locator('text=Queue Depth')).toBeVisible();
      await expect(page.locator('text=Error Rate')).toBeVisible();
    });

    test('should display system health', async ({ page }) => {
      // Navigate to Logs & Monitoring tab
      await page.locator('[role="tab"]').nth(2).click();

      // Check system health section
      await expect(page.locator('text=System Health')).toBeVisible();

      // Check health status indicators
      const healthIndicators = page.locator('[data-testid="health-indicator"]').or(
        page.locator('text=Healthy').or(page.locator('text=Warning')).or(page.locator('text=Error'))
      );
      await expect(healthIndicators.first()).toBeVisible();
    });
  });

  test.describe('Error Handling', () => {
    test('should display error messages appropriately', async ({ page }) => {
      // Simulate network error by going offline
      await page.context().setOffline(true);

      // Try to refresh data
      await page.locator('[role="tab"]').nth(0).click();
      const refreshButton = page.locator('button:has-text("Refresh")');
      await refreshButton.click();

      // Should show error message
      await expect(page.locator('[role="alert"]')).toBeVisible({ timeout: 5000 });

      // Go back online
      await page.context().setOffline(false);
    });
  });

  test.describe('Responsive Design', () => {
    test('should work on mobile viewport', async ({ page }) => {
      // Set mobile viewport
      await page.setViewportSize({ width: 375, height: 667 });

      // Check that navigation is still accessible
      await expect(page.locator('[role="tab"]').first()).toBeVisible();

      // Check that main content is visible
      await expect(page.locator('h1')).toBeVisible();
    });

    test('should work on tablet viewport', async ({ page }) => {
      // Set tablet viewport
      await page.setViewportSize({ width: 768, height: 1024 });

      // Check that all tabs are visible
      await expect(page.locator('[role="tab"]')).toHaveCount(3);

      // Check that cards are properly laid out
      await expect(page.locator('text=Crawler Status')).toBeVisible();
    });
  });

  test.describe('Accessibility', () => {
    test('should have proper ARIA labels', async ({ page }) => {
      // Check for proper ARIA labels on tabs
      await expect(page.locator('[role="tab"]').first()).toHaveAttribute('aria-controls');

      // Check for proper ARIA labels on buttons
      const buttons = page.locator('button');
      const buttonCount = await buttons.count();

      for (let i = 0; i < Math.min(buttonCount, 5); i++) {
        const button = buttons.nth(i);
        const hasAriaLabel = await button.getAttribute('aria-label');
        const hasText = await button.textContent();

        // Button should have either aria-label or visible text
        expect(hasAriaLabel || hasText?.trim()).toBeTruthy();
      }
    });

    test('should support keyboard navigation', async ({ page }) => {
      // Test tab navigation
      await page.keyboard.press('Tab');
      await page.keyboard.press('Tab');
      await page.keyboard.press('Tab');

      // Should be able to navigate through interface
      const focusedElement = page.locator(':focus');
      await expect(focusedElement).toBeVisible();
    });
  });
});

// Visual regression tests
test.describe('Visual Regression Tests', () => {
  test('dashboard should match expected appearance', async ({ page }) => {
    await page.goto('http://localhost:3001');
    await page.locator('[role="tab"]').nth(0).click();

    // Take screenshot of dashboard
    await expect(page).toHaveScreenshot('crawler-dashboard.png');
  });

  test('configuration page should match expected appearance', async ({ page }) => {
    await page.goto('http://localhost:3001');
    await page.locator('[role="tab"]').nth(1).click();

    // Take screenshot of configuration page
    await expect(page).toHaveScreenshot('crawler-configuration.png');
  });

  test('logs page should match expected appearance', async ({ page }) => {
    await page.goto('http://localhost:3001');
    await page.locator('[role="tab"]').nth(2).click();

    // Take screenshot of logs page
    await expect(page).toHaveScreenshot('crawler-logs.png');
  });
});
