import { test, expect } from '@playwright/test';

test('debug main element content constraints', async ({ page }) => {
  await page.goto('/');

  // Wait for the page to load
  await page.waitForLoadState('networkidle');

  // Set desktop viewport
  await page.setViewportSize({ width: 1200, height: 800 });

  // Get the main element
  const main = page.locator('main');
  await expect(main).toBeVisible();

  // Debug the content inside the main element
  const contentAnalysis = await main.evaluate((el) => {
    const children = Array.from(el.children);

    const analyzeChild = (child: Element, level: number = 0): any => {
      const computed = window.getComputedStyle(child);
      const rect = child.getBoundingClientRect();

      const childInfo = {
        level,
        tagName: child.tagName,
        className: child.className,
        id: child.id,
        width: computed.width,
        maxWidth: computed.maxWidth,
        minWidth: computed.minWidth,
        boxSizing: computed.boxSizing,
        display: computed.display,
        position: computed.position,
        actualWidth: rect.width,
        actualX: rect.x,
        children: Array.from(child.children).map(grandchild => analyzeChild(grandchild, level + 1))
      };

      return childInfo;
    };

    return {
      mainElement: {
        width: window.getComputedStyle(el).width,
        actualWidth: el.getBoundingClientRect().width,
        children: children.map(child => analyzeChild(child))
      }
    };
  });

  console.log('=== MAIN ELEMENT CONTENT ANALYSIS ===');
  console.log(`Main element width: ${contentAnalysis.mainElement.width}`);
  console.log(`Main element actual width: ${contentAnalysis.mainElement.actualWidth}`);

  const printChild = (child: any, level: number = 0) => {
    const indent = '  '.repeat(level);
    console.log(`${indent}<${child.tagName}> class="${child.className}" id="${child.id}"`);
    console.log(`${indent}  width: ${child.width}, maxWidth: ${child.maxWidth}, minWidth: ${child.minWidth}`);
    console.log(`${indent}  actualWidth: ${child.actualWidth}, actualX: ${child.actualX}`);
    console.log(`${indent}  display: ${child.display}, position: ${child.position}`);

    if (child.children && child.children.length > 0) {
      console.log(`${indent}  Children:`);
      child.children.forEach((grandchild: any) => printChild(grandchild, level + 1));
    }
  };

  console.log('\nChildren:');
  contentAnalysis.mainElement.children.forEach(child => printChild(child));

  // Check if any child has width constraints that might affect the main element
  const findWidthConstraints = (child: any, path: string = ''): any[] => {
    const constraints = [];
    const currentPath = path ? `${path} > ${child.tagName}` : child.tagName;

    if (child.width !== 'auto' && child.width !== '100%' && child.width !== 'inherit') {
      constraints.push({
        path: currentPath,
        className: child.className,
        width: child.width,
        maxWidth: child.maxWidth,
        minWidth: child.minWidth
      });
    }

    if (child.children) {
      child.children.forEach((grandchild: any) => {
        constraints.push(...findWidthConstraints(grandchild, currentPath));
      });
    }

    return constraints;
  };

  const widthConstraints = contentAnalysis.mainElement.children.flatMap(child => findWidthConstraints(child));

  if (widthConstraints.length > 0) {
    console.log('\n=== WIDTH CONSTRAINTS FOUND ===');
    widthConstraints.forEach((constraint, index) => {
      console.log(`${index + 1}. ${constraint.path} class="${constraint.className}"`);
      console.log(`   width: ${constraint.width}`);
      if (constraint.maxWidth !== 'none') console.log(`   maxWidth: ${constraint.maxWidth}`);
      if (constraint.minWidth !== 'auto') console.log(`   minWidth: ${constraint.minWidth}`);
    });
  } else {
    console.log('\n=== NO WIDTH CONSTRAINTS FOUND ===');
  }
});
