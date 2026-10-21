/**
 * REQUIREMENT: The About page shows the version that was built, not a hard-coded one.
 * PURPOSE: Regression test for ECO-404 (the 4.0.0 build said "Version 3.7.2").
 */

import React from 'react';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { screen } from '@testing-library/react';
import { renderWithProviders } from '../../test-utils/test-providers';
import About from '../About';

const packageVersion: string = JSON.parse(
  readFileSync(resolve(import.meta.dirname, '../../../package.json'), 'utf8')
).version;

describe('About', () => {
  it("shows package.json's version", () => {
    renderWithProviders(<About />);

    expect(screen.getByText(`Version ${packageVersion}`)).toBeInTheDocument();
  });

  it('has no version number written into the page source', () => {
    const source = readFileSync(resolve(import.meta.dirname, '../About.tsx'), 'utf8');

    expect(source).not.toMatch(/Version\s+\d+\.\d+/);
  });
});
