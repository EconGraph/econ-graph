import { test, expect } from '@playwright/test';

test.describe('Test Sidebar Fix', () => {
  test('check if sidebar is now visible and accessible', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Take a screenshot to see what's actually visible
    await page.screenshot({ path: 'sidebar-debug.png', fullPage: true });

    // Check if sidebar is visible using new accessibility attributes
    // Handle both desktop and mobile implementations
    const desktopSidebar = page.locator('[data-testid="sidebar-desktop"]');
    const mobileSidebar = page.locator('.MuiDrawer-paper');

    const isDesktop = await desktopSidebar.isVisible();
    const isMobile = await mobileSidebar.isVisible();

    if (isDesktop) {
      console.log('Desktop sidebar detected');
      await expect(desktopSidebar).toBeVisible();
    } else if (isMobile) {
      console.log('Mobile sidebar detected');
      await expect(mobileSidebar).toBeVisible();
    } else {
      // Try to open mobile sidebar if it's not visible
      const menuButton = page.locator('[aria-label="Open navigation menu"]');
      if (await menuButton.isVisible()) {
        await menuButton.click();
        await page.waitForTimeout(500);
        await expect(mobileSidebar).toBeVisible();
        console.log('Mobile sidebar opened and visible');
      }
    }

    console.log('Sidebar visible:', isDesktop || isMobile);

    // Check sidebar content structure
    const sidebarContent = page.locator('[data-testid="sidebar-content"]');
    await expect(sidebarContent).toBeVisible();
    console.log('Sidebar content visible:', await sidebarContent.isVisible());

    // Check sidebar header
    const sidebarHeader = page.locator('[data-testid="sidebar-header"]');
    await expect(sidebarHeader).toBeVisible();
    console.log('Sidebar header visible:', await sidebarHeader.isVisible());

    // Check sidebar title
    const sidebarTitle = page.locator('[data-testid="sidebar-title"]');
    await expect(sidebarTitle).toBeVisible();
    await expect(sidebarTitle).toHaveText('EconGraph');
    console.log('Sidebar title visible:', await sidebarTitle.isVisible());

    // Check navigation sections
    const primaryNav = page.locator('[data-testid="sidebar-primary-nav"]');
    await expect(primaryNav).toBeVisible();
    console.log('Primary navigation visible:', await primaryNav.isVisible());

    const secondaryNav = page.locator('[data-testid="sidebar-secondary-nav"]');
    await expect(secondaryNav).toBeVisible();
    console.log('Secondary navigation visible:', await secondaryNav.isVisible());

    // Check specific navigation items using new data-testid attributes
    const dashboardNav = page.locator('[data-testid="sidebar-nav-dashboard"]');
    await expect(dashboardNav).toBeVisible();
    console.log('Dashboard nav visible:', await dashboardNav.isVisible());

    const aboutNav = page.locator('[data-testid="sidebar-nav-about"]');
    await expect(aboutNav).toBeVisible();
    console.log('About nav visible:', await aboutNav.isVisible());

    // Test navigation functionality
    await aboutNav.click();
    await page.waitForTimeout(500);

    // Verify navigation worked
    expect(page.url()).toContain('/about');
    console.log('Successfully navigated to About page!');

    // Test keyboard navigation
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    const exploreNav = page.locator('[data-testid="sidebar-nav-explore-series"]');
    await expect(exploreNav).toBeVisible();

    // Focus and use keyboard navigation
    await exploreNav.focus();
    await page.keyboard.press('Enter');
    await page.waitForTimeout(500);

    expect(page.url()).toContain('/explore');
    console.log('Successfully navigated to Explore page using keyboard!');
  });

  test('check sidebar accessibility attributes', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    // Check that navigation items have proper ARIA attributes
    const aboutNav = page.locator('[data-testid="sidebar-nav-about"]');
    await expect(aboutNav).toHaveAttribute('role', 'button');
    await expect(aboutNav).toHaveAttribute('tabindex', '0');
    await expect(aboutNav).toHaveAttribute('aria-label', 'Navigate to About: About EconGraph');

    // Check that current page is marked with aria-current
    const dashboardNav = page.locator('[data-testid="sidebar-nav-dashboard"]');
    await expect(dashboardNav).toHaveAttribute('aria-current', 'page');

    console.log('All accessibility attributes are properly set!');
  });
});
