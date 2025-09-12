import { test, expect } from '@playwright/test';

test('debug calc width calculation', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Debug the calc() calculation
  const calcDebug = await main.evaluate((el) => {
    const computed = window.getComputedStyle(el);
    const boundingBox = el.getBoundingClientRect();

    // Get the parent element
    const parent = el.parentElement;
    const parentComputed = parent ? window.getComputedStyle(parent) : null;
    const parentBoundingBox = parent ? parent.getBoundingClientRect() : null;

    // Check what 100% would be in different contexts
    const viewportWidth = window.innerWidth;
    const documentWidth = document.documentElement.scrollWidth;
    const bodyWidth = document.body.scrollWidth;

    return {
      element: {
        width: computed.width,
        boundingBox: {
          width: boundingBox.width,
          x: boundingBox.x
        }
      },
      parent: parent ? {
        width: parentComputed!.width,
        boundingBox: {
          width: parentBoundingBox!.width,
          x: parentBoundingBox!.x
        }
      } : null,
      contexts: {
        viewportWidth,
        documentWidth,
        bodyWidth
      },
      calculations: {
        '100% of viewport - 240px': viewportWidth - 240,
        '100% of parent - 240px': parentBoundingBox ? parentBoundingBox.width - 240 : null,
        '100% of document - 240px': documentWidth - 240,
        '100% of body - 240px': bodyWidth - 240
      }
    };
  });

  console.log('=== CALC WIDTH DEBUG ===');
  console.log('Element width:', calcDebug.element.width);
  console.log('Element bounding box:', JSON.stringify(calcDebug.element.boundingBox));

  if (calcDebug.parent) {
    console.log('Parent width:', calcDebug.parent.width);
    console.log('Parent bounding box:', JSON.stringify(calcDebug.parent.boundingBox));
  }

  console.log('Context widths:');
  console.log('  Viewport:', calcDebug.contexts.viewportWidth);
  console.log('  Document:', calcDebug.contexts.documentWidth);
  console.log('  Body:', calcDebug.contexts.bodyWidth);

  console.log('Calculations:');
  Object.entries(calcDebug.calculations).forEach(([desc, value]) => {
    console.log(`  ${desc}: ${value}`);
  });

  // Let's also test what happens if we manually set different width values
  const widthTests = await main.evaluate((el) => {
    const originalWidth = el.style.width;
    const results = [];

    // Test different width values
    const testWidths = [
      '960px',
      'calc(100vw - 240px)',
      'calc(100% - 240px)',
      'auto'
    ];

    testWidths.forEach(testWidth => {
      el.style.width = testWidth;
      const computed = window.getComputedStyle(el);
      const boundingBox = el.getBoundingClientRect();

      results.push({
        testWidth,
        computedWidth: computed.width,
        boundingBoxWidth: boundingBox.width
      });
    });

    // Restore original
    el.style.width = originalWidth;

    return results;
  });

  console.log('\n=== WIDTH TESTS ===');
  widthTests.forEach(test => {
    console.log(`${test.testWidth}: computed=${test.computedWidth}, boundingBox=${test.boundingBoxWidth}`);
  });
});
