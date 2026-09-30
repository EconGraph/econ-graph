// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import React from 'react';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import MultiCountryDashboard from '../MultiCountryDashboard';

describe('MultiCountryDashboard', () => {
  it('renders the header info icon as a tooltip target, not a dead button (ECO-246)', () => {
    render(<MultiCountryDashboard />);

    // A dead IconButton (no onClick/href) would show up here; the header has no other
    // interactive controls, so this catches a regression to the old <IconButton><Info /></IconButton>.
    expect(screen.queryAllByRole('button')).toHaveLength(0);
    // The icon still carries the tooltip text as its own accessible name (role="img"),
    // so it isn't hidden from assistive tech now that it's not a button.
    expect(
      screen.getByRole('img', {
        name: 'Compare economic indicators across multiple countries with interactive charts and analysis',
      })
    ).toBeInTheDocument();
  });
});
