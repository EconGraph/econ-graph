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
 * Elements are matched by name, not by where they are imported from, so a local `Button` is
 * checked too, and `import { Button as MuiButton }` is not.
 */

/** The components the rule checks, by the name they are used under. */
const BUTTONS = new Set(['Button', 'IconButton', 'ListItemButton']);

/** Props other than event handlers that give a button something to do. */
const ACTIONS = new Set(['href', 'component', 'to']);

/** An event handler prop: `onClick`, `onMouseDown`, `onKeyDown`, ... */
const HANDLER = /^on[A-Z]/;

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
        const live = node.attributes.some(
          attr =>
            attr.type === 'JSXSpreadAttribute' ||
            (attr.name.type === 'JSXIdentifier' &&
              (ACTIONS.has(attr.name.name) ||
                HANDLER.test(attr.name.name) ||
                (attr.name.name === 'type' && hasFormAction(attr))))
        );
        if (!live) context.report({ node, messageId: 'dead', data: { name: node.name.name } });
      },
    };
  },
};

export default rule;
