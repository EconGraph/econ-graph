import { test, expect } from '@playwright/test';

test('debug flex container behavior', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the parent flex container
  const parentContainer = page.locator('main').locator('..');
  await expect(parentContainer).toBeVisible();

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Debug the flex container
  const flexInfo = await parentContainer.evaluate((el) => {
    const computed = window.getComputedStyle(el);
    const boundingBox = el.getBoundingClientRect();

    return {
      tagName: el.tagName,
      className: el.className,
      display: computed.display,
      flexDirection: computed.flexDirection,
      flexWrap: computed.flexWrap,
      justifyContent: computed.justifyContent,
      alignItems: computed.alignItems,
      width: computed.width,
      height: computed.height,
      boundingBox: {
        width: boundingBox.width,
        height: boundingBox.height,
        x: boundingBox.x,
        y: boundingBox.y
      },
      children: Array.from(el.children).map(child => ({
        tagName: child.tagName,
        className: child.className,
        width: window.getComputedStyle(child).width,
        flexGrow: window.getComputedStyle(child).flexGrow,
        flexShrink: window.getComputedStyle(child).flexShrink,
        flexBasis: window.getComputedStyle(child).flexBasis,
        boundingBox: child.getBoundingClientRect()
      }))
    };
  });

  console.log('=== FLEX CONTAINER INFO ===');
  console.log(`Container: <${flexInfo.tagName}> class="${flexInfo.className}"`);
  console.log(`Display: ${flexInfo.display}`);
  console.log(`Flex Direction: ${flexInfo.flexDirection}`);
  console.log(`Width: ${flexInfo.width}`);
  console.log(`Bounding Box: ${JSON.stringify(flexInfo.boundingBox)}`);

  console.log('\n=== CHILDREN ===');
  flexInfo.children.forEach((child, index) => {
    console.log(`${index}. <${child.tagName}> class="${child.className}"`);
    console.log(`   width: ${child.width}`);
    console.log(`   flexGrow: ${child.flexGrow}, flexShrink: ${child.flexShrink}, flexBasis: ${child.flexBasis}`);
    console.log(`   boundingBox: ${JSON.stringify(child.boundingBox)}`);
  });

  // Check if there are any CSS rules that might be constraining the flex behavior
  const mainElement = await main.evaluate((el) => {
    const computed = window.getComputedStyle(el);

    // Check if there are any conflicting width rules
    const allWidthRules = [];

    // Check inline styles
    if (el.style.width) {
      allWidthRules.push({
        source: 'inline',
        width: el.style.width,
        priority: 'highest'
      });
    }

    // Check computed styles
    allWidthRules.push({
      source: 'computed',
      width: computed.width,
      priority: 'final'
    });

    return {
      allWidthRules,
      flexProperties: {
        flexGrow: computed.flexGrow,
        flexShrink: computed.flexShrink,
        flexBasis: computed.flexBasis,
        flex: computed.flex
      }
    };
  });

  console.log('\n=== MAIN ELEMENT WIDTH RULES ===');
  mainElement.allWidthRules.forEach((rule, index) => {
    console.log(`${index}. ${rule.source}: ${rule.width} (${rule.priority})`);
  });

  console.log('\n=== MAIN ELEMENT FLEX PROPERTIES ===');
  console.log(JSON.stringify(mainElement.flexProperties, null, 2));

  // Let's also check what happens if we force the width
  const forcedWidth = await main.evaluate((el) => {
    // Temporarily set a fixed width to see if it works
    const originalWidth = el.style.width;
    el.style.width = '960px';

    const computed = window.getComputedStyle(el);
    const boundingBox = el.getBoundingClientRect();

    // Restore original
    el.style.width = originalWidth;

    return {
      forcedWidth: computed.width,
      forcedBoundingBox: {
        width: boundingBox.width,
        x: boundingBox.x
      }
    };
  });

  console.log('\n=== FORCED WIDTH TEST ===');
  console.log(`Forced width: ${forcedWidth.forcedWidth}`);
  console.log(`Forced bounding box: ${JSON.stringify(forcedWidth.forcedBoundingBox)}`);
});
