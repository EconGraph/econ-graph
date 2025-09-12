import { test, expect } from '@playwright/test';

test('debug main element width constraints', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Get all parent elements and their styles
  const parentInfo = await main.evaluate((el) => {
    const parents = [];
    let current = el.parentElement;
    let level = 0;

    while (current && level < 10) {
      const computed = window.getComputedStyle(current);
      const boundingBox = current.getBoundingClientRect();

      parents.push({
        level,
        tagName: current.tagName,
        className: current.className,
        id: current.id,
        width: computed.width,
        maxWidth: computed.maxWidth,
        minWidth: computed.minWidth,
        boxSizing: computed.boxSizing,
        display: computed.display,
        flexGrow: computed.flexGrow,
        flexShrink: computed.flexShrink,
        flexBasis: computed.flexBasis,
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

    return parents;
  });

  console.log('=== PARENT ELEMENTS ===');
  parentInfo.forEach((parent, index) => {
    console.log(`${index}. <${parent.tagName}> class="${parent.className}" id="${parent.id}"`);
    console.log(`   width: ${parent.width}, maxWidth: ${parent.maxWidth}, minWidth: ${parent.minWidth}`);
    console.log(`   display: ${parent.display}, flexGrow: ${parent.flexGrow}, flexShrink: ${parent.flexShrink}`);
    console.log(`   boundingBox: ${JSON.stringify(parent.boundingBox)}`);
    console.log('');
  });

  // Get the main element's computed styles
  const mainStyles = await main.evaluate((el) => {
    const computed = window.getComputedStyle(el);
    const boundingBox = el.getBoundingClientRect();

    return {
      width: computed.width,
      maxWidth: computed.maxWidth,
      minWidth: computed.minWidth,
      boxSizing: computed.boxSizing,
      display: computed.display,
      flexGrow: computed.flexGrow,
      flexShrink: computed.flexShrink,
      flexBasis: computed.flexBasis,
      marginLeft: computed.marginLeft,
      marginRight: computed.marginRight,
      paddingLeft: computed.paddingLeft,
      paddingRight: computed.paddingRight,
      boundingBox: {
        width: boundingBox.width,
        height: boundingBox.height,
        x: boundingBox.x,
        y: boundingBox.y
      }
    };
  });

  console.log('=== MAIN ELEMENT STYLES ===');
  console.log(JSON.stringify(mainStyles, null, 2));

  // Check if there are any CSS rules affecting the main element
  const cssRules = await main.evaluate((el) => {
    const rules = [];
    let current = el;

    while (current && current !== document.body) {
      const computed = window.getComputedStyle(current);
      if (computed.width !== 'auto' && computed.width !== '100%') {
        rules.push({
          element: current.tagName,
          className: current.className,
          width: computed.width,
          maxWidth: computed.maxWidth
        });
      }
      current = current.parentElement;
    }

    return rules;
  });

  console.log('=== ELEMENTS WITH WIDTH CONSTRAINTS ===');
  cssRules.forEach((rule, index) => {
    console.log(`${index}. <${rule.element}> class="${rule.className}"`);
    console.log(`   width: ${rule.width}, maxWidth: ${rule.maxWidth}`);
  });
});
