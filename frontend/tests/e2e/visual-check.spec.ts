import { test, expect } from '@playwright/test';

test.describe('Visual Check', () => {
  test('check sidebar visually', async ({ page }) => {
    await page.goto('http://localhost/');
    await page.waitForLoadState('networkidle');

    // Take a screenshot to see what's actually visible
    await page.screenshot({ path: 'visual-check.png', fullPage: true });

    // Check if sidebar is visible
    const sidebar = page.locator('[role="navigation"][aria-label="Main navigation"]');
    console.log('Sidebar visible:', await sidebar.isVisible());

    // Check sidebar dimensions and position
    const sidebarBox = await sidebar.boundingBox();
    console.log('Sidebar bounding box:', sidebarBox);

    // Check for any buttons in the sidebar
    const sidebarButtons = await sidebar.locator('button').all();
    console.log('Sidebar button count:', sidebarButtons.length);

    for (let i = 0; i < sidebarButtons.length; i++) {
      const button = sidebarButtons[i];
      const text = await button.textContent();
      const isVisible = await button.isVisible();
      const box = await button.boundingBox();
      console.log(
        `  Sidebar Button ${i}: "${text}" (visible: ${isVisible}, box: ${JSON.stringify(box)})`
      );
    }

    // Check for any clickable elements (buttons, links, etc.)
    const clickableElements = await sidebar.locator('button, a, [role="button"], [onclick]').all();
    console.log('Clickable elements count:', clickableElements.length);

    for (let i = 0; i < clickableElements.length; i++) {
      const element = clickableElements[i];
      const tagName = await element.evaluate(el => el.tagName);
      const text = await element.textContent();
      const isVisible = await element.isVisible();
      const box = await element.boundingBox();
      console.log(
        `  Clickable Element ${i}: <${tagName}> "${text}" (visible: ${isVisible}, box: ${JSON.stringify(box)})`
      );
    }

    // Check for ListItemButton specifically
    const listItemButtons = await sidebar.locator('[role="button"]').all();
    console.log('ListItemButton count:', listItemButtons.length);

    for (let i = 0; i < listItemButtons.length; i++) {
      const button = listItemButtons[i];
      const text = await button.textContent();
      const isVisible = await button.isVisible();
      const box = await button.boundingBox();
      console.log(
        `  ListItemButton ${i}: "${text}" (visible: ${isVisible}, box: ${JSON.stringify(box)})`
      );
    }

    // Check for any text content in the sidebar
    const sidebarText = await sidebar.textContent();
    console.log('Sidebar text content:', sidebarText);

    // Check for the red test div
    try {
      const testDiv = await page.locator('div:has-text("TEST SIDEBAR VISIBLE")').first();
      const testDivVisible = await testDiv.isVisible({ timeout: 5000 });
      const testDivBox = await testDiv.boundingBox();
      console.log('Test div visible:', testDivVisible);
      console.log('Test div bounding box:', testDivBox);
    } catch (error) {
      console.log('Test div not found or not visible');
    }

    // Check for console errors
    const consoleLogs = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleLogs.push(`Console Error: ${msg.text()}`);
      }
    });

    // Wait a bit to catch any console errors
    await page.waitForTimeout(2000);
    if (consoleLogs.length > 0) {
      console.log('Console errors:', consoleLogs);
    }
  });
});
