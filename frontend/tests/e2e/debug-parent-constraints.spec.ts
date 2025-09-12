import { test, expect } from '@playwright/test';

test('debug parent container constraints', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the main element and its parents
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Debug the parent chain
  const parentChain = await main.evaluate((el) => {
    const chain = [];
    let current = el;
    let level = 0;

    while (current && level < 10) {
      const computed = window.getComputedStyle(current);
      const boundingBox = current.getBoundingClientRect();

      chain.push({
        level,
        tagName: current.tagName,
        className: current.className,
        id: current.id,
        width: computed.width,
        maxWidth: computed.maxWidth,
        minWidth: computed.minWidth,
        boxSizing: computed.boxSizing,
        display: computed.display,
        position: computed.position,
        overflow: computed.overflow,
        overflowX: computed.overflowX,
        overflowY: computed.overflowY,
        paddingLeft: computed.paddingLeft,
        paddingRight: computed.paddingRight,
        marginLeft: computed.marginLeft,
        marginRight: computed.marginRight,
        boundingBox: {
          width: boundingBox.width,
          height: boundingBox.height,
          x: boundingBox.x,
          y: boundingBox.y
        }
      });

      current = current.parentElement;
      level++;
    }

    return chain;
  });

  console.log('=== PARENT CHAIN ANALYSIS ===');
  parentChain.forEach((element, index) => {
    console.log(`${index}. Level ${element.level}: <${element.tagName}> class="${element.className}" id="${element.id}"`);
    console.log(`   width: ${element.width}, maxWidth: ${element.maxWidth}, minWidth: ${element.minWidth}`);
    console.log(`   display: ${element.display}, position: ${element.position}`);
    console.log(`   boxSizing: ${element.boxSizing}`);
    console.log(`   padding: ${element.paddingLeft} ${element.paddingRight}`);
    console.log(`   margin: ${element.marginLeft} ${element.marginRight}`);
    console.log(`   overflow: ${element.overflow} (x: ${element.overflowX}, y: ${element.overflowY})`);
    console.log(`   boundingBox: ${JSON.stringify(element.boundingBox)}`);
    console.log('');
  });

  // Check if any parent has constraints that would affect the 100% calculation
  const constraints = parentChain.filter(el =>
    el.maxWidth !== 'none' ||
    el.overflow !== 'visible' ||
    el.width !== 'auto' && el.width !== '100%'
  );

  if (constraints.length > 0) {
    console.log('=== POTENTIAL CONSTRAINTS ===');
    constraints.forEach((constraint, index) => {
      console.log(`${index + 1}. <${constraint.tagName}> class="${constraint.className}"`);
      console.log(`   width: ${constraint.width}`);
      console.log(`   maxWidth: ${constraint.maxWidth}`);
      console.log(`   overflow: ${constraint.overflow}`);
    });
  } else {
    console.log('=== NO OBVIOUS CONSTRAINTS FOUND ===');
  }

  // Let's also check the viewport and document dimensions
  const viewportInfo = await page.evaluate(() => {
    return {
      viewport: {
        width: window.innerWidth,
        height: window.innerHeight
      },
      document: {
        width: document.documentElement.scrollWidth,
        height: document.documentElement.scrollHeight
      },
      body: {
        width: document.body.scrollWidth,
        height: document.body.scrollHeight
      }
    };
  });

  console.log('\n=== VIEWPORT INFO ===');
  console.log(JSON.stringify(viewportInfo, null, 2));
});
