import { test, expect } from '@playwright/test';

test('debug CSS specificity and width overrides', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Get all CSS rules that might be affecting the width
  const cssAnalysis = await main.evaluate((el) => {
    const rules = [];

    // Get all stylesheets
    for (let i = 0; i < document.styleSheets.length; i++) {
      try {
        const sheet = document.styleSheets[i];
        if (sheet.cssRules) {
          for (let j = 0; j < sheet.cssRules.length; j++) {
            const rule = sheet.cssRules[j];
            if (rule.type === CSSRule.STYLE_RULE) {
              const styleRule = rule as CSSStyleRule;
              try {
                // Check if this rule applies to our element and has width-related properties
                if (el.matches(styleRule.selectorText)) {
                  const hasWidthProps = styleRule.style.width ||
                                       styleRule.style.maxWidth ||
                                       styleRule.style.minWidth ||
                                       styleRule.style.flexBasis ||
                                       styleRule.style.flexGrow ||
                                       styleRule.style.flexShrink;

                  if (hasWidthProps) {
                    rules.push({
                      source: sheet.href || 'inline',
                      selector: styleRule.selectorText,
                      specificity: calculateSpecificity(styleRule.selectorText),
                      cssText: styleRule.cssText,
                      width: styleRule.style.width,
                      maxWidth: styleRule.style.maxWidth,
                      minWidth: styleRule.style.minWidth,
                      flexBasis: styleRule.style.flexBasis,
                      flexGrow: styleRule.style.flexGrow,
                      flexShrink: styleRule.style.flexShrink
                    });
                  }
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

    // Sort by specificity (highest first)
    rules.sort((a, b) => b.specificity - a.specificity);

    return rules;
  });

  // Helper function to calculate CSS specificity
  function calculateSpecificity(selector: string): number {
    let specificity = 0;
    const parts = selector.split(/\s+/);

    parts.forEach(part => {
      // IDs
      const ids = (part.match(/#/g) || []).length;
      specificity += ids * 1000;

      // Classes, attributes, pseudo-classes
      const classes = (part.match(/\./g) || []).length;
      const attributes = (part.match(/\[/g) || []).length;
      const pseudoClasses = (part.match(/:/g) || []).length;
      specificity += (classes + attributes + pseudoClasses) * 100;

      // Elements, pseudo-elements
      const elements = part.replace(/[#\.\[\]:]/g, '').split(/(?=[A-Z])/).length;
      const pseudoElements = (part.match(/::/g) || []).length;
      specificity += (elements + pseudoElements) * 1;
    });

    return specificity;
  }

  console.log('=== CSS RULES AFFECTING MAIN ELEMENT (sorted by specificity) ===');
  cssAnalysis.forEach((rule, index) => {
    console.log(`${index + 1}. Specificity: ${rule.specificity}`);
    console.log(`   Source: ${rule.source}`);
    console.log(`   Selector: ${rule.selector}`);
    if (rule.width) console.log(`   width: ${rule.width}`);
    if (rule.maxWidth) console.log(`   maxWidth: ${rule.maxWidth}`);
    if (rule.minWidth) console.log(`   minWidth: ${rule.minWidth}`);
    if (rule.flexBasis) console.log(`   flexBasis: ${rule.flexBasis}`);
    if (rule.flexGrow) console.log(`   flexGrow: ${rule.flexGrow}`);
    if (rule.flexShrink) console.log(`   flexShrink: ${rule.flexShrink}`);
    console.log(`   CSS: ${rule.cssText}`);
    console.log('');
  });

  // Also check if there are any CSS custom properties or other constraints
  const computedStyles = await main.evaluate((el) => {
    const computed = window.getComputedStyle(el);
    return {
      width: computed.width,
      maxWidth: computed.maxWidth,
      minWidth: computed.minWidth,
      flexBasis: computed.flexBasis,
      flexGrow: computed.flexGrow,
      flexShrink: computed.flexShrink,
      flex: computed.flex,
      boxSizing: computed.boxSizing,
      display: computed.display,
      position: computed.position
    };
  });

  console.log('=== COMPUTED STYLES ===');
  console.log(JSON.stringify(computedStyles, null, 2));
});
