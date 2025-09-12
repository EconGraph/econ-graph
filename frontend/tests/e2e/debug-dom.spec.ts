import { test, expect } from '@playwright/test';

test('debug DOM structure to see what elements exist', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Log the entire HTML structure of main
  const mainHTML = await main.innerHTML();
  console.log('=== MAIN ELEMENT HTML ===');
  console.log(mainHTML);

  // Get all elements with MUI classes
  const muiElements = await page.locator('[class*="Mui"]').all();
  console.log(`\n=== FOUND ${muiElements.length} MUI ELEMENTS ===`);

  for (let i = 0; i < Math.min(muiElements.length, 10); i++) {
    const element = muiElements[i];
    const tagName = await element.evaluate(el => el.tagName);
    const className = await element.getAttribute('class');
    const text = await element.textContent();
    console.log(`${i + 1}. <${tagName.toLowerCase()}> class="${className}" text="${text?.substring(0, 50)}..."`);
  }

  // Check for any container-like elements
  const containerElements = await page.locator('[class*="container"], [class*="Container"]').all();
  console.log(`\n=== FOUND ${containerElements.length} CONTAINER ELEMENTS ===`);

  for (let i = 0; i < containerElements.length; i++) {
    const element = containerElements[i];
    const tagName = await element.evaluate(el => el.tagName);
    const className = await element.getAttribute('class');
    const boundingBox = await element.boundingBox();
    console.log(`${i + 1}. <${tagName.toLowerCase()}> class="${className}" bounds=${JSON.stringify(boundingBox)}`);
  }

  // Check the main element's computed styles
  const mainStyles = await main.evaluate((el) => {
    const computed = window.getComputedStyle(el);
    return {
      width: computed.width,
      height: computed.height,
      marginLeft: computed.marginLeft,
      paddingLeft: computed.paddingLeft,
      position: computed.position,
      display: computed.display,
      flexGrow: computed.flexGrow,
    };
  });

  console.log('\n=== MAIN ELEMENT COMPUTED STYLES ===');
  console.log(JSON.stringify(mainStyles, null, 2));

  // Get bounding box of main element
  const mainBounds = await main.boundingBox();
  console.log('\n=== MAIN ELEMENT BOUNDING BOX ===');
  console.log(JSON.stringify(mainBounds, null, 2));
});
