import { test, expect } from '@playwright/test';

test('debug header positioning and flex layout', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get all the elements
  const header = page.locator('header');
  const sidebar = page.locator('[role="navigation"][aria-label="Main navigation"]');
  const main = page.locator('main');
  const flexContainer = page.locator('main').locator('..');

  await expect(header).toBeVisible();
  await expect(sidebar).toBeVisible();
  await expect(main).toBeVisible();
  await expect(flexContainer).toBeVisible();

  // Get positioning info for all elements
  const positioningInfo = await page.evaluate(() => {
    const header = document.querySelector('header');
    const sidebar = document.querySelector('[role="navigation"][aria-label="Main navigation"]');
    const main = document.querySelector('main');
    const flexContainer = main?.parentElement;

    const getElementInfo = (el: Element | null) => {
      if (!el) return null;
      const computed = window.getComputedStyle(el);
      const boundingBox = el.getBoundingClientRect();

      return {
        tagName: el.tagName,
        className: el.className,
        position: computed.position,
        top: computed.top,
        left: computed.left,
        right: computed.right,
        bottom: computed.bottom,
        zIndex: computed.zIndex,
        width: computed.width,
        height: computed.height,
        display: computed.display,
        flexGrow: computed.flexGrow,
        flexShrink: computed.flexShrink,
        flexBasis: computed.flexBasis,
        boundingBox: {
          x: boundingBox.x,
          y: boundingBox.y,
          width: boundingBox.width,
          height: boundingBox.height
        }
      };
    };

    return {
      flexContainer: getElementInfo(flexContainer),
      header: getElementInfo(header),
      sidebar: getElementInfo(sidebar),
      main: getElementInfo(main)
    };
  });

  console.log('=== FLEX CONTAINER ===');
  console.log(JSON.stringify(positioningInfo.flexContainer, null, 2));

  console.log('\n=== HEADER ===');
  console.log(JSON.stringify(positioningInfo.header, null, 2));

  console.log('\n=== SIDEBAR ===');
  console.log(JSON.stringify(positioningInfo.sidebar, null, 2));

  console.log('\n=== MAIN ===');
  console.log(JSON.stringify(positioningInfo.main, null, 2));

  // Check if the header is affecting the flex layout
  const flexLayoutAnalysis = await page.evaluate(() => {
    const flexContainer = document.querySelector('main')?.parentElement;
    if (!flexContainer) return null;

    const children = Array.from(flexContainer.children);
    const totalFlexBasis = children.reduce((sum, child) => {
      const computed = window.getComputedStyle(child);
      const flexBasis = computed.flexBasis;
      const flexGrow = parseFloat(computed.flexGrow) || 0;
      const flexShrink = parseFloat(computed.flexShrink) || 0;

      let basisValue = 0;
      if (flexBasis === 'auto') {
        basisValue = child.getBoundingClientRect().width;
      } else if (flexBasis.includes('px')) {
        basisValue = parseFloat(flexBasis);
      }

      return {
        ...sum,
        [child.tagName]: {
          flexBasis: flexBasis,
          flexGrow: flexGrow,
          flexShrink: flexShrink,
          basisValue: basisValue,
          actualWidth: child.getBoundingClientRect().width
        }
      };
    }, {} as any);

    const containerWidth = flexContainer.getBoundingClientRect().width;
    const totalBasis = Object.values(totalFlexBasis).reduce((sum: number, child: any) => sum + child.basisValue, 0);
    const totalGrow = Object.values(totalFlexBasis).reduce((sum: number, child: any) => sum + child.flexGrow, 0);

    return {
      containerWidth,
      totalBasis,
      totalGrow,
      children: totalFlexBasis
    };
  });

  console.log('\n=== FLEX LAYOUT ANALYSIS ===');
  console.log(`Container width: ${flexLayoutAnalysis?.containerWidth}`);
  console.log(`Total basis: ${flexLayoutAnalysis?.totalBasis}`);
  console.log(`Total grow: ${flexLayoutAnalysis?.totalGrow}`);
  console.log('Children:');
  Object.entries(flexLayoutAnalysis?.children || {}).forEach(([tag, info]: [string, any]) => {
    console.log(`  ${tag}: basis=${info.basisValue}, grow=${info.flexGrow}, actual=${info.actualWidth}`);
  });
});
