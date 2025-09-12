import { test, expect } from '@playwright/test';

test('debug inline styles and CSS specificity', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Debug inline styles and CSS specificity
  const styleAnalysis = await main.evaluate((el) => {
    const computed = window.getComputedStyle(el);
    const inlineStyles = el.style.cssText;

    // Get all CSS rules that apply to this element
    const rules = [];

    // Check inline styles
    if (inlineStyles) {
      rules.push({
        type: 'inline',
        source: 'style attribute',
        cssText: inlineStyles,
        width: el.style.width,
        specificity: 1000 // Inline styles have highest specificity
      });
    }

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
                if (el.matches(styleRule.selectorText)) {
                  // Calculate specificity
                  let specificity = 0;
                  const selector = styleRule.selectorText;

                  // IDs
                  const ids = (selector.match(/#/g) || []).length;
                  specificity += ids * 1000;

                  // Classes, attributes, pseudo-classes
                  const classes = (selector.match(/\./g) || []).length;
                  const attributes = (selector.match(/\[/g) || []).length;
                  const pseudoClasses = (selector.match(/:/g) || []).length;
                  specificity += (classes + attributes + pseudoClasses) * 100;

                  // Elements, pseudo-elements
                  const elements = selector.replace(/[#\.\[\]:]/g, '').split(/(?=[A-Z])/).length;
                  const pseudoElements = (selector.match(/::/g) || []).length;
                  specificity += (elements + pseudoElements) * 1;

                  rules.push({
                    type: 'stylesheet',
                    source: sheet.href || 'inline',
                    selector: styleRule.selectorText,
                    cssText: styleRule.cssText,
                    width: styleRule.style.width,
                    specificity: specificity
                  });
                }
              } catch (e) {
                continue;
              }
            }
          }
        }
      } catch (e) {
        continue;
      }
    }

    // Sort by specificity (highest first)
    rules.sort((a, b) => b.specificity - a.specificity);

    return {
      computedStyles: {
        width: computed.width,
        maxWidth: computed.maxWidth,
        minWidth: computed.minWidth,
        flexBasis: computed.flexBasis,
        flexGrow: computed.flexGrow,
        flexShrink: computed.flexShrink
      },
      inlineStyles: inlineStyles,
      rules: rules
    };
  });

  console.log('=== COMPUTED STYLES ===');
  console.log(JSON.stringify(styleAnalysis.computedStyles, null, 2));

  console.log('\n=== INLINE STYLES ===');
  console.log(styleAnalysis.inlineStyles || 'No inline styles');

  console.log('\n=== CSS RULES (sorted by specificity) ===');
  styleAnalysis.rules.forEach((rule, index) => {
    console.log(`${index + 1}. Specificity: ${rule.specificity}`);
    console.log(`   Type: ${rule.type}`);
    console.log(`   Source: ${rule.source}`);
    if (rule.selector) {
      console.log(`   Selector: ${rule.selector}`);
    }
    if (rule.width) {
      console.log(`   width: ${rule.width}`);
    }
    console.log(`   CSS: ${rule.cssText}`);
    console.log('');
  });

  // Also check if there are any CSS custom properties or other constraints
  const elementInfo = await main.evaluate((el) => {
    return {
      tagName: el.tagName,
      className: el.className,
      id: el.id,
      style: el.style.cssText,
      attributes: Array.from(el.attributes).map(attr => ({
        name: attr.name,
        value: attr.value
      }))
    };
  });

  console.log('\n=== ELEMENT INFO ===');
  console.log(`Tag: ${elementInfo.tagName}`);
  console.log(`Class: ${elementInfo.className}`);
  console.log(`ID: ${elementInfo.id}`);
  console.log(`Style: ${elementInfo.style}`);
  console.log('Attributes:');
  elementInfo.attributes.forEach(attr => {
    console.log(`  ${attr.name}: ${attr.value}`);
  });
});
