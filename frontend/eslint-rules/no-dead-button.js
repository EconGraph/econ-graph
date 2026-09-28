// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

/**
 * Local ESLint rule: a MUI button must do something when clicked.
 *
 * Flags a `Button`, `IconButton` or `ListItemButton` element with no event handler (`onClick`,
 * `onMouseDown`, any `on…` prop), no `href`, `component` or `to`, no `type` other than 'button',
 * and no spread props that could supply one. Such a button renders, takes focus and does nothing,
 * which the release crawl (REL-4, ECO-246) found several of.
 *
 * A button that is the child of an `asChild` element (a Radix `SheetTrigger`, `DrawerTrigger`, ...)
 * gets its handler from that parent, so it is not flagged.
 *
 * Elements are matched by name, not by where they are imported from, so a local `Button` is
 * checked too, and `import { Button as MuiButton }` is not.
 */

/** The components the rule checks, by the name they are used under. */
const BUTTONS = new Set(['Button', 'IconButton', 'ListItemButton']);

/** Props other than event handlers that give a button something to do. */
const ACTIONS = new Set(['href', 'to']);

/** An event handler prop: `onClick`, `onMouseDown`, `onKeyDown`, ... */
const HANDLER = /^on[A-Z]/;

/**
 * Whether a `component` attribute names a component that can act, such as a router `Link`. A
 * string (`component='span'`) only changes the root element, so it doesn't count.
 * @param {import('estree-jsx').JSXAttribute} attr - The `component` attribute.
 * @returns {boolean} True unless the value is a string literal.
 */
function hasComponentAction(attr) {
  const value = attr.value;
  const component = value?.type === 'JSXExpressionContainer' ? value.expression : value;
  return Boolean(component) && component.type !== 'Literal';
}

/**
 * Whether a `type` attribute gives the button a form action.
 * @param {import('estree-jsx').JSXAttribute} attr - The `type` attribute.
 * @returns {boolean} False only for a literal `type='button'` (or no value); a computed type
 *   may be 'submit', so it counts.
 */
function hasFormAction(attr) {
  const value = attr.value;
  if (!value) return false;
  const literal =
    value.type === 'Literal'
      ? value
      : value.type === 'JSXExpressionContainer' && value.expression.type === 'Literal'
        ? value.expression
        : null;
  return literal ? literal.value !== 'button' : true;
}

/**
 * Whether the button's parent element passes its props down with `asChild`.
 * @param {import('estree-jsx').JSXOpeningElement} node - The button's opening element.
 * @returns {boolean} True when the enclosing JSX element has an `asChild` attribute.
 */
function triggeredByParent(node) {
  const parent = node.parent?.parent;
  return (
    parent?.type === 'JSXElement' &&
    parent.openingElement.attributes.some(
      attr => attr.type === 'JSXAttribute' && attr.name.name === 'asChild'
    )
  );
}

/** @type {import('eslint').Rule.RuleModule} */
const rule = {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Require Button, IconButton and ListItemButton to have an event handler, href, component, to, or a form type',
    },
    schema: [],
    messages: {
      dead: '{{name}} does nothing when clicked: give it onClick (or another handler), href, component/to or type="submit", or remove it.',
    },
  },
  create(context) {
    return {
      JSXOpeningElement(node) {
        if (node.name.type !== 'JSXIdentifier' || !BUTTONS.has(node.name.name)) return;
        if (triggeredByParent(node)) return;
        const live = node.attributes.some(
          attr =>
            attr.type === 'JSXSpreadAttribute' ||
            (attr.name.type === 'JSXIdentifier' &&
              (ACTIONS.has(attr.name.name) ||
                (attr.name.name === 'component' && hasComponentAction(attr)) ||
                HANDLER.test(attr.name.name) ||
                (attr.name.name === 'type' && hasFormAction(attr))))
        );
        if (!live) context.report({ node, messageId: 'dead', data: { name: node.name.name } });
      },
    };
  },
};

export default rule;
