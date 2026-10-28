# Accessibility Testing Guidelines: ARIA Labels, Roles, and Data Test IDs

Use native HTML to give controls their semantics, accessible names to describe their purpose, and semantic queries to test what users can find. `data-testid` provides a test hook when those queries are unsuitable; it does not make an element accessible. The examples below illustrate these choices for frontend contributors and component-test authors.

## 📋 **Quick Reference Table**

| Attribute | Purpose | When to Use | When to Avoid | Impact on Accessibility |
|-----------|---------|-------------|---------------|------------------------|
| `aria-label` | Provides accessible name | Controls without a visible label | When visible text already supplies the name | Supplies the name announced by assistive technology |
| `role` | Defines element function | When native HTML cannot express the needed semantics | Redundant roles on native elements | Exposes semantics; does not implement behavior |
| `data-testid` | Testing selector | When semantic queries are unsuitable | Replacing a missing role or accessible name | None - testing only |

## 🏷️ **ARIA Labels (`aria-label`)**

### **Purpose**
Provides an accessible name for elements that lack visible text, enabling assistive technologies to announce the element's purpose to users.

### **When to Use**

#### ✅ **Appropriate Use Cases**

1. **Icon-only buttons**
```html
<button aria-label="Close dialog">
  <svg aria-hidden="true" focusable="false">...</svg>
</button>
```

2. **Image inside an icon-only control**
```html
<button type="button" aria-label="Search">
  <img src="search-icon.svg" alt="" />
</button>
```

The button supplies the action and its name, so its image is decorative and has empty `alt`. An informative image outside a control instead needs meaningful `alt` text; see the [WAI image guidance](https://www.w3.org/WAI/tutorials/images/).

3. **Form controls without visible labels**
```html
<input type="search" aria-label="Search products" placeholder="Enter search term" />
```

4. **Icon-only disclosure button**
```html
<button type="button" aria-label="Expand section" aria-expanded="false" aria-controls="details">
  <svg aria-hidden="true" focusable="false">...</svg>
</button>
<div id="details" hidden>Section details</div>
```

The disclosure example shows the collapsed state. Its activation handler must reveal the content and update `aria-expanded`. Prefer a visible `<label>` for form controls where the design allows one.

#### ❌ **When to Avoid**

1. **Elements whose visible text already supplies a suitable name**
```html
<!-- Avoid an unnecessary name override -->
<button aria-label="Submit form">Submit</button>

<!-- ✅ GOOD: Let visible text speak for itself -->
<button>Submit</button>
```

2. **Over-labeling obvious elements**
```html
<!-- ❌ BAD: Unnecessary noise -->
<h1 aria-label="Main heading">Welcome</h1>

<!-- ✅ GOOD: Native semantics are sufficient -->
<h1>Welcome</h1>
```

3. **Using for testing purposes**
```html
<!-- ❌ BAD: Misusing accessibility attributes for testing -->
<button aria-label="test-submit-button">Submit</button>

<!-- Visible text supplies the name; query by role and name in tests -->
<button>Submit</button>
```

### **Best Practices**

- **Test with screen readers**: Verify that `aria-label` improves the experience
- **Keep labels concise**: Avoid verbose descriptions
- **Use consistent terminology**: Maintain consistent language across the application
- **Consider context**: Ensure labels make sense in the surrounding context

## 🎭 **Roles (`role`)**

### **Purpose**
Defines the function or purpose of an element to assistive technologies when native HTML semantics are insufficient. A role does not add event handlers, keyboard interaction, or focus management.

### **When to Use**

#### ✅ **Appropriate Use Cases**

1. **Custom widgets when native HTML is insufficient**

Prefer `<button type="button">Save</button>` over a custom button. If a custom element must use `role="button"`, it also needs focusability, a name, visible focus, pointer activation, and activation with both Enter and Space. Follow the [APG button pattern](https://www.w3.org/WAI/ARIA/apg/patterns/button/), including where focus moves after activation.

2. **Landmarks on non-semantic containers**
```html
<div role="navigation" aria-label="Primary">
  <!-- Navigation links -->
</div>
```

Use `<nav aria-label="Primary">` when you control the markup; the explicit role is useful when the container cannot be changed.

3. **Form controls**

A fixed list of options usually needs a native select:

```html
<label for="country">Country</label>
<select id="country" name="country">
  <option value="us">United States</option>
  <option value="ca">Canada</option>
</select>
```

If the interaction requires a custom combobox, follow the [APG combobox pattern](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/). Put the role and accessible name on the operative control, connect it to its popup with `aria-controls`, and keep expanded and selection states synchronized. Implement the pattern's keyboard and focus behavior as well as its attributes.

4. **Custom dialogs and modals**

This fragment illustrates the role and naming relationship, not a complete modal implementation:

```html
<div role="dialog" aria-labelledby="dialog-title" aria-modal="true">
  <h2 id="dialog-title">Confirm action</h2>
  <button type="button">Cancel</button>
</div>
```

A modal must move focus inside when opened, contain Tab and Shift+Tab navigation, close on Escape, and return focus appropriately when closed. The background must be inert while it is open. `aria-modal="true"` alone does none of this; follow the [APG modal dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/).

#### ❌ **When to Avoid**

1. **Native semantic elements**
```html
<!-- ❌ BAD: Redundant role on native element -->
<button role="button">Click me</button>

<!-- ✅ GOOD: Native semantics are sufficient -->
<button>Click me</button>
```

2. **Over-specifying roles**
```html
<!-- ❌ BAD: Unnecessary role specification -->
<nav role="navigation">
  <ul role="list">
    <li role="listitem">Item</li>
  </ul>
</nav>

<!-- ✅ GOOD: Native semantics provide roles automatically -->
<nav>
  <ul>
    <li>Item</li>
  </ul>
</nav>
```

### **Best Practices**

- **Prefer native HTML elements**: Use semantic HTML before adding roles
- **Use ARIA roles sparingly**: Only when native semantics are insufficient
- **Test with assistive technologies**: Verify roles improve accessibility
- **Follow ARIA patterns**: Use established ARIA patterns for complex components

## 🧪 **Data Test IDs (`data-testid`)**

### **Purpose**
Provides stable selectors for automated testing without affecting accessibility or user experience.

### **When to Use**

#### ✅ **Appropriate Use Cases**

1. **A noninteractive target without useful semantic selection**
```html
<div data-testid="chart-plot-surface"></div>
```

A test may need this drawing surface to inspect rendering dimensions. Give the chart itself an accessible name and an appropriate text or table alternative separately; this hook is not a complete chart implementation.

2. **A container for scoping repeated content**
```html
<section data-testid="primary-series-card">
  <h2>Primary series</h2>
  <button type="button">Remove</button>
</section>
```

Prefer a named region or other user-facing context when that fits the content. A test ID can scope a query when the container has no suitable semantic identity; still query the button inside by role and name.

3. **Form validation hooks**

Hooks can coexist with labels and error associations. This fragment shows one invalid state, not a complete validation flow; the form must also manage validation timing and notify users when errors appear.

```html
<label for="email">Email</label>
<input id="email" data-testid="email-input" type="email"
       aria-invalid="true" aria-describedby="email-error" />
<p id="email-error" data-testid="email-error-message">Invalid email</p>
```

Query the input by its label and the error by text where possible.

#### ❌ **When to Avoid**

1. **Using for accessibility purposes**
```html
<!-- BAD: The test ID supplies no accessible name -->
<button data-testid="close-button"><svg aria-hidden="true"></svg></button>

<!-- GOOD: Name the control; a test hook may coexist when needed -->
<button aria-label="Close" data-testid="close-button">
  <svg aria-hidden="true" focusable="false">...</svg>
</button>
```

2. **Over-relying on data-testid**
```html
<!-- ❌ BAD: Too many test IDs clutter the markup -->
<div data-testid="container">
  <div data-testid="header">
    <h1 data-testid="title">Title</h1>
  </div>
</div>

<!-- ✅ GOOD: Use semantic selectors when possible -->
<div>
  <header>
    <h1>Title</h1>
  </header>
</div>
```

### **Best Practices**

- **Use sparingly**: Only when semantic selectors aren't sufficient
- **Keep names descriptive**: Use clear, meaningful test IDs
- **Avoid implementation details**: Don't tie test IDs to CSS classes or internal structure
- **Document non-obvious hooks**: Explain why a test needs a test ID instead of a semantic query

## 🎯 **Decision Framework**

### **Step 1: Check for Native Semantics**
```html
<!-- ✅ GOOD: Use native semantic elements -->
<button>Submit</button>
<nav>Navigation</nav>
<main>Main content</main>

<!-- ❌ AVOID: Adding roles to native elements -->
<button role="button">Submit</button>
<nav role="navigation">Navigation</nav>
```

### **Step 2: Assess Accessibility Needs**
```html
<!-- Element needs accessible name? -->
<button aria-label="Close dialog">×</button>

<!-- A native button provides the role and keyboard activation -->
<button type="button">Expand section</button>

<!-- A drawing surface may need a test hook -->
<div data-testid="chart-plot-surface"></div>
```

### **Step 3: Choose the Right Attribute**

| Scenario | Recommended Attribute | Example |
|----------|----------------------|---------|
| Icon-only button | `aria-label` | `<button aria-label="Close">×</button>` |
| Custom widget | APG role, states, name, and keyboard/focus behavior | Follow the widget pattern linked above |
| Nonsemantic rendering target | `data-testid` | `<div data-testid="chart-plot-surface"></div>` |
| Form input without label | `aria-label` | `<input aria-label="Search" />` |
| Native semantic element | No redundant role; controls still need a name | `<button>Submit</button>` |

## 🚫 **Common Anti-Patterns**

### **1. ARIA Label Overkill**
```html
<!-- ❌ BAD: Unnecessary aria-label on visible text -->
<h1 aria-label="Welcome to our website">Welcome to our website</h1>
<p aria-label="This is a paragraph">This is a paragraph</p>

<!-- ✅ GOOD: Let visible text speak for itself -->
<h1>Welcome to our website</h1>
<p>This is a paragraph</p>
```

### **2. Role Redundancy**
```html
<!-- ❌ BAD: Redundant roles on native elements -->
<button role="button">Click me</button>
<nav role="navigation">Menu</nav>

<!-- ✅ GOOD: Native semantics are sufficient -->
<button>Click me</button>
<nav>Menu</nav>
```

### **3. Inventing Accessible Names for Tests**
```html
<!-- ❌ BAD: Using aria-label for testing -->
<button aria-label="test-submit-button">Submit</button>

<!-- GOOD: Visible text supplies the name -->
<button>Submit</button>
```

Querying the user-facing accessible name is encouraged. Do not replace it with a machine-only identifier; add a test ID only when a semantic query is unsuitable.

### **4. Over-Engineering Simple Elements**
```html
<!-- ❌ BAD: Unnecessary complexity -->
<div role="button" tabindex="0" aria-label="Simple button" data-testid="simple-button">
  Click me
</div>

<!-- ✅ GOOD: Use native button element -->
<button>Click me</button>
```

## 🧪 **Testing Considerations**

### **React Testing Library Best Practices**

The snippets in this section assume Vitest's `test`/`expect`, React Testing Library's `render`/`screen`/`within`, and the repository's jest-dom setup. Import components under test from their modules. For interaction tests, import `userEvent` from `@testing-library/user-event` and create `const user = userEvent.setup()` inside each test.

#### **1. Prefer Semantic Selectors**
```typescript
const submitButton = screen.getByRole('button', { name: /submit/i });
const searchInput = screen.getByLabelText(/search/i);
```

A failed role/name query should fail the test: the control may have lost its semantics, or the query may be ambiguous. For repeated controls, scope the query with `within` or use a more specific accessible name. Do not catch the failure and retry by test ID.

#### **2. Choose a Test ID Deliberately**
```typescript
const plotSurface = screen.getByTestId('chart-plot-surface');
const card = screen.getByTestId('primary-series-card');
const removeButton = within(card).getByRole('button', { name: 'Remove' });
```

These select the nonsemantic surface and container illustrated above. Choosing a test ID is a decision about the target, not a runtime fallback after an expected semantic query fails.

#### **3. Test the Resulting Accessible Name**
```tsx
test('names the close button', () => {
  render(<button type="button" aria-label="Close">×</button>);
  expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument();
});
```

### **Material UI Specific Considerations**

#### **1. Select Component Testing**

Use one visible label and connect it with `labelId`, as in the [MUI Select accessibility guidance](https://mui.com/material-ui/react-select/#accessibility):

```tsx
import { FormControl, InputLabel, MenuItem, Select } from '@mui/material';

test('names the country select', () => {
  render(
    <FormControl>
      <InputLabel id="country-label">Country</InputLabel>
      <Select id="country" labelId="country-label" label="Country" defaultValue="us">
        <MenuItem value="us">United States</MenuItem>
        <MenuItem value="ca">Canada</MenuItem>
      </Select>
    </FormControl>
  );
  expect(screen.getByRole('combobox', { name: 'Country' })).toBeInTheDocument();
});
```

#### **2. Dialog Testing**

For a component that opens a modal named “Confirm action”, test the role and name as well as interactions:

```tsx
test('opens the named modal', async () => {
  const user = userEvent.setup();
  render(<DialogComponent />);
  await user.click(screen.getByRole('button', { name: /open dialog/i }));
  const dialog = await screen.findByRole('dialog', { name: 'Confirm action' });
  expect(dialog).toHaveAttribute('aria-modal', 'true');
});
```

This assertion covers only the exposed role, name, and modal state. Add tests for initial focus, keyboard navigation, Escape, and focus restoration. Verify the interaction with a keyboard and screen reader in a browser too.

## 🛠️ **Testing Tools and Frontend Setup**

### **Existing Static Analysis**

The [frontend manifest](../../frontend/package.json) includes ESLint 9 and `eslint-plugin-jsx-a11y`. The [flat ESM configuration](../../frontend/eslint.config.js) registers the plugin and spreads its recommended rules, then overrides some rules, including disabling label association and selected interaction rules. Review the effective configuration when deciding what lint covers; it does not enforce every recommendation in this guide.

From `frontend`, run the existing command:

```bash
npm run lint
```

If changing accessibility lint rules, edit the existing flat configuration. Do not add a legacy `.eslintrc.js` or assume that passing lint establishes accessibility.

### **Optional DOM Scans in Vitest**

The frontend uses [Vitest with JSDOM](../../frontend/vitest.config.ts); its [setup file](../../frontend/src/setupTests.vitest.ts) loads `@testing-library/jest-dom/vitest` and performs cleanup. `axe-core` is not a frontend dependency. To add component scans, install it as a development dependency from `frontend` and commit the resulting manifest and lockfile changes with the integration:

```bash
npm install --save-dev axe-core
```

The following self-contained test can be saved as `src/__tests__/accessibility.test.tsx` after that addition. It uses explicit imports and Vitest's built-in equality assertion, so it needs no custom matcher registration or `jest-axe` types.

```tsx
// @vitest-environment jsdom
import React from 'react';
import { afterEach, expect, test, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import axe from 'axe-core';

afterEach(cleanup);

test('accessibility: scan a named button', async () => {
  vi.useRealTimers();
  const { container } = render(
    <main>
      <button type="button" aria-label="Close">×</button>
    </main>
  );
  expect(screen.getByRole('button', { name: 'Close' })).toBeTruthy();
  const results = await axe.run(container, {
    rules: { 'color-contrast': { enabled: false } },
  });
  expect(results.violations).toEqual([]);
});
```

Use real timers because axe schedules asynchronous work. Contrast checking is disabled here because JSDOM cannot supply the rendering information it needs. For components that render portals, scan `baseElement` from `render` so the scan includes their content. Inspect reported violations and incomplete results; a scan with no violations only describes the rules and DOM state tested. Browser checks are still needed for contrast, layout, and interaction.

Run the example from `frontend` with the existing test script:

```bash
npm test -- src/__tests__/accessibility.test.tsx
```

This is an optional integration recipe, not an existing accessibility suite or package script. The [axe API](https://github.com/dequelabs/axe-core/blob/develop/doc/API.md) documents scan options and results. Do not use `@axe-core/react` for this React 19 frontend: its [maintainer documentation](https://github.com/dequelabs/axe-core-npm/tree/develop/packages/react#react-support) excludes React 18 and later. That adapter instruments a development browser console; it is separate from a component-test DOM scan.

### **Coverage and Manual Evaluation**

| Tool or method | Type | What it checks | Limits |
|----------------|------|----------------|--------|
| `eslint-plugin-jsx-a11y` | Static JSX analysis | Configured markup rules | Cannot observe rendered behavior |
| `axe-core` | Automated DOM scan | Detectable violations in the scanned state | Partial coverage; JSDOM lacks visual rendering |
| React Testing Library | Automated component tests | Queries and interactions asserted by the test | Only covers authored scenarios |
| Keyboard and screen-reader evaluation | Manual browser testing | Focus, announcements, navigation, and task completion | Requires representative tasks and assistive technologies |

Automated checks do not establish WCAG conformance. Use them alongside manual evaluation: navigate without a pointer, inspect visible focus and focus order, exercise dialogs and validation, and check names and announcements with assistive technologies. Include zoom, reflow, and contrast checks in browser evaluation.

### **CI and Local Checks**

The [accessibility workflow](../../.github/workflows/accessibility-tests.yml) runs on manual dispatch or a weekly schedule. It is separate from PR-triggered checks; a generated manual checklist is guidance for evaluation, not evidence that someone performed it. Assess the actual job commands and results before relying on a run as validation.

For a future scan integration, use the existing `npm run lint` and `npm test -- <test-file>` commands from `frontend` in local checks and CI. A hook or job must install the declared dependencies, use the repository's Node version, and fail when a required command fails. There are no existing `lint:a11y` or `test:a11y` package scripts to invoke.

## 📚 **References and Resources**

### **Official Documentation**
- [ARIA: aria-label attribute - MDN](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Attributes/aria-label)
- [ARIA: document structural roles - MDN](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/structural_roles)
- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [React Testing Library Queries](https://testing-library.com/docs/queries/about/)

### **Testing Tools**
- [eslint-plugin-jsx-a11y](https://github.com/jsx-eslint/eslint-plugin-jsx-a11y) - Static accessibility linting
- [axe-core](https://github.com/dequelabs/axe-core) - Automated accessibility rules for rendered DOM
- [Vitest environments](https://vitest.dev/guide/environment.html) - JSDOM and browser environment differences

### **Best Practice Guides**
- [ARIA Labels and Descriptions - WebAIM](https://webaim.org/techniques/aria/)
- [Accessibility Developer Guide](https://www.accessibility-developer-guide.com/)
- [Testing Library User Events](https://testing-library.com/docs/user-event/intro/)

### **Native HTML and ARIA Patterns**
- [WAI image tutorials](https://www.w3.org/WAI/tutorials/images/)
- [APG button pattern](https://www.w3.org/WAI/ARIA/apg/patterns/button/)
- [APG combobox pattern](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/)
- [APG modal dialog pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)

## 🎯 **Summary Guidelines**

### **Quick Decision Tree**

1. **Does the element have visible text?**
   - Yes → Prefer that text as the accessible name
   - No → Supply an accessible name for controls; prefer a visible label where possible

2. **Is it a native semantic element?**
   - Yes → Avoid adding `role` attributes
   - No → Prefer a native element; otherwise implement the full applicable ARIA pattern

3. **Do you need a testing selector?**
   - Yes → Choose `data-testid` only when semantic queries are unsuitable
   - No → Focus on accessibility attributes only

### **Key Principles**

- **Accessibility First**: Always prioritize user accessibility over testing convenience
- **Native Semantics**: Use semantic HTML before ARIA attributes
- **Minimal ARIA**: Only add ARIA when native semantics are insufficient
- **Semantic Queries**: Test user-facing roles and names; reserve test IDs for targets without suitable semantic queries
- **User Testing**: Test with actual assistive technologies to verify improvements
