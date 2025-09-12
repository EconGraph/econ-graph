import { test, expect } from '@playwright/test';

test('debug CSS rules affecting main element', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Get all CSS rules affecting the main element
  const cssRules = await main.evaluate((el) => {
    const rules = [];

    // Get computed styles
    const computed = window.getComputedStyle(el);

    // Check if there are any inline styles
    if (el.style.cssText) {
      rules.push({
        type: 'inline',
        source: 'style attribute',
        cssText: el.style.cssText
      });
    }

    // Get all stylesheets and find rules that match this element
    for (let i = 0; i < document.styleSheets.length; i++) {
      try {
        const sheet = document.styleSheets[i];
        if (sheet.cssRules) {
          for (let j = 0; j < sheet.cssRules.length; j++) {
            const rule = sheet.cssRules[j];
            if (rule.type === CSSRule.STYLE_RULE) {
              const styleRule = rule as CSSStyleRule;
              try {
                // Check if this rule applies to our element
                if (el.matches(styleRule.selectorText)) {
                  rules.push({
                    type: 'stylesheet',
                    source: sheet.href || 'inline stylesheet',
                    selector: styleRule.selectorText,
                    cssText: styleRule.cssText,
                    width: styleRule.style.width,
                    maxWidth: styleRule.style.maxWidth,
                    minWidth: styleRule.style.minWidth,
                    flexGrow: styleRule.style.flexGrow,
                    flexShrink: styleRule.style.flexShrink,
                    flexBasis: styleRule.style.flexBasis
                  });
                }
              } catch (e) {
                // Some selectors might not be valid for matches()
                continue;
              }
            }
          }
        }
      } catch (e) {
        // Cross-origin stylesheets might throw errors
        continue;
      }
    }

    return {
      computedStyles: {
        width: computed.width,
        maxWidth: computed.maxWidth,
        minWidth: computed.minWidth,
        flexGrow: computed.flexGrow,
        flexShrink: computed.flexShrink,
        flexBasis: computed.flexBasis,
        display: computed.display,
        boxSizing: computed.boxSizing
      },
      rules: rules
    };
  });

  console.log('=== COMPUTED STYLES ===');
  console.log(JSON.stringify(cssRules.computedStyles, null, 2));

  console.log('\n=== CSS RULES AFFECTING MAIN ELEMENT ===');
  cssRules.rules.forEach((rule, index) => {
    console.log(`${index}. ${rule.type.toUpperCase()} - ${rule.source}`);
    if (rule.selector) {
      console.log(`   Selector: ${rule.selector}`);
    }
    if (rule.width) console.log(`   width: ${rule.width}`);
    if (rule.maxWidth) console.log(`   maxWidth: ${rule.maxWidth}`);
    if (rule.minWidth) console.log(`   minWidth: ${rule.minWidth}`);
    if (rule.flexGrow) console.log(`   flexGrow: ${rule.flexGrow}`);
    if (rule.flexShrink) console.log(`   flexShrink: ${rule.flexShrink}`);
    if (rule.flexBasis) console.log(`   flexBasis: ${rule.flexBasis}`);
    console.log(`   CSS: ${rule.cssText}`);
    console.log('');
  });

  // Also check the specific MUI Box styles
  const boxStyles = await main.evaluate((el) => {
    const computed = window.getComputedStyle(el);
    return {
      className: el.className,
      allProperties: Array.from(computed).map(prop => ({
        property: prop,
        value: computed.getPropertyValue(prop)
      })).filter(prop =>
        prop.property.includes('width') ||
        prop.property.includes('flex') ||
        prop.property.includes('display') ||
        prop.property.includes('box-sizing')
      )
    };
  });

  console.log('\n=== MUI BOX SPECIFIC STYLES ===');
  console.log(`Class: ${boxStyles.className}`);
  boxStyles.allProperties.forEach(prop => {
    console.log(`${prop.property}: ${prop.value}`);
  });
});
