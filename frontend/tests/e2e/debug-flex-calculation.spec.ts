import { test, expect } from '@playwright/test';

test('debug flex calculation in detail', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the flex container and its children
  const flexContainer = page.locator('main').locator('..');
  const sidebar = page.locator('[role="navigation"][aria-label="Main navigation"]');
  const main = page.locator('main');

  await expect(flexContainer).toBeVisible();
  await expect(sidebar).toBeVisible();
  await expect(main).toBeVisible();

  // Debug the flex calculation step by step
  const flexCalculation = await page.evaluate(() => {
    const container = document.querySelector('main')?.parentElement;
    if (!container) return null;

    const children = Array.from(container.children);
    const containerRect = container.getBoundingClientRect();

    const childInfo = children.map(child => {
      const computed = window.getComputedStyle(child);
      const rect = child.getBoundingClientRect();

      return {
        tagName: child.tagName,
        className: child.className,
        flexGrow: parseFloat(computed.flexGrow) || 0,
        flexShrink: parseFloat(computed.flexShrink) || 0,
        flexBasis: computed.flexBasis,
        width: computed.width,
        actualWidth: rect.width,
        actualX: rect.x
      };
    });

    // Calculate what the flex algorithm should do
    const totalGrow = childInfo.reduce((sum, child) => sum + child.flexGrow, 0);
    const totalShrink = childInfo.reduce((sum, child) => sum + child.flexShrink, 0);

    // Calculate available space
    const totalBasis = childInfo.reduce((sum, child) => {
      if (child.flexBasis === 'auto') {
        return sum + child.actualWidth;
      } else if (child.flexBasis.includes('px')) {
        return sum + parseFloat(child.flexBasis);
      }
      return sum;
    }, 0);

    const availableSpace = containerRect.width - totalBasis;

    return {
      container: {
        width: containerRect.width,
        computedWidth: window.getComputedStyle(container).width
      },
      children: childInfo,
      calculation: {
        totalGrow,
        totalShrink,
        totalBasis,
        availableSpace,
        expectedMainWidth: childInfo.find(c => c.tagName === 'MAIN')?.actualWidth + (availableSpace * (childInfo.find(c => c.tagName === 'MAIN')?.flexGrow || 0) / totalGrow)
      }
    };
  });

  console.log('=== FLEX CALCULATION DEBUG ===');
  console.log('Container:');
  console.log(`  width: ${flexCalculation?.container.width}`);
  console.log(`  computed width: ${flexCalculation?.container.computedWidth}`);

  console.log('\nChildren:');
  flexCalculation?.children.forEach((child, index) => {
    console.log(`${index}. <${child.tagName}> class="${child.className}"`);
    console.log(`   flexGrow: ${child.flexGrow}, flexShrink: ${child.flexShrink}, flexBasis: ${child.flexBasis}`);
    console.log(`   width: ${child.width}, actualWidth: ${child.actualWidth}, actualX: ${child.actualX}`);
  });

  console.log('\nCalculation:');
  console.log(`  totalGrow: ${flexCalculation?.calculation.totalGrow}`);
  console.log(`  totalShrink: ${flexCalculation?.calculation.totalShrink}`);
  console.log(`  totalBasis: ${flexCalculation?.calculation.totalBasis}`);
  console.log(`  availableSpace: ${flexCalculation?.calculation.availableSpace}`);
  console.log(`  expectedMainWidth: ${flexCalculation?.calculation.expectedMainWidth}`);

  // Let's also try to understand why the flex calculation is wrong
  const flexDebug = await page.evaluate(() => {
    const container = document.querySelector('main')?.parentElement;
    if (!container) return null;

    // Check if there are any CSS constraints on the container
    const containerComputed = window.getComputedStyle(container);
    const containerRect = container.getBoundingClientRect();

    // Check if the container has any width constraints
    const hasWidthConstraints = containerComputed.width !== 'auto' &&
                               containerComputed.width !== '100%' &&
                               containerComputed.maxWidth !== 'none';

    return {
      container: {
        width: containerComputed.width,
        maxWidth: containerComputed.maxWidth,
        minWidth: containerComputed.minWidth,
        boxSizing: containerComputed.boxSizing,
        display: containerComputed.display,
        flexDirection: containerComputed.flexDirection,
        flexWrap: containerComputed.flexWrap,
        hasWidthConstraints,
        actualWidth: containerRect.width
      }
    };
  });

  console.log('\n=== CONTAINER CONSTRAINTS ===');
  console.log(`width: ${flexDebug?.container.width}`);
  console.log(`maxWidth: ${flexDebug?.container.maxWidth}`);
  console.log(`minWidth: ${flexDebug?.container.minWidth}`);
  console.log(`boxSizing: ${flexDebug?.container.boxSizing}`);
  console.log(`display: ${flexDebug?.container.display}`);
  console.log(`flexDirection: ${flexDebug?.container.flexDirection}`);
  console.log(`flexWrap: ${flexDebug?.container.flexWrap}`);
  console.log(`hasWidthConstraints: ${flexDebug?.container.hasWidthConstraints}`);
  console.log(`actualWidth: ${flexDebug?.container.actualWidth}`);
});
