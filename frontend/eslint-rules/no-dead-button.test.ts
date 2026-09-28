// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import typescriptParser from '@typescript-eslint/parser';
import { RuleTester } from 'eslint';
import { afterAll, describe, it } from 'vitest';

import noDeadButton from './no-dead-button.js';

RuleTester.afterAll = afterAll;
RuleTester.describe = describe;
RuleTester.it = it;

const tester = new RuleTester({
  languageOptions: {
    parser: typescriptParser,
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

tester.run('no-dead-button', noDeadButton, {
  valid: [
    '<Button onClick={save}>Save</Button>',
    "<Button href='/about'>About</Button>",
    '<Button component={Link} to="/explore">Explore</Button>',
    "<Button type='submit'>Send</Button>",
    "<Button type={'submit'}>Send</Button>",
    "<Button type='reset'>Clear</Button>",
    "<Button type={isLast ? 'submit' : 'button'}>Next</Button>",
    '<IconButton onMouseDown={keepFocus} aria-label="show password" />',
    '<IconButton {...props} />',
    '<ListItemButton onClick={() => go(item)} />',
    // Other components are not checked.
    '<Chip label="GDP" />',
    '<Foo.Button />',
  ],
  invalid: [
    { code: '<Button>View Details</Button>', errors: [{ messageId: 'dead' }] },
    { code: '<IconButton size="small" />', errors: [{ messageId: 'dead' }] },
    { code: '<ListItemButton selected />', errors: [{ messageId: 'dead' }] },
    { code: "<Button type='button'>Refresh</Button>", errors: [{ messageId: 'dead' }] },
    { code: "<Button type={'button'} size='small'>Refresh</Button>", errors: [{ messageId: 'dead' }] },
  ],
});
