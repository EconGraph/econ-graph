/**
 * REQUIREMENT: Unknown URLs show a clear message instead of an empty page.
 * PURPOSE: Test the catch-all not-found page.
 */

import React from 'react';
import { screen } from '@testing-library/react';
import { renderWithProviders } from '../../test-utils/test-providers';
import NotFound from '../NotFound';

describe('NotFound', () => {
  it('shows a not-found message and a link back to the dashboard', () => {
    renderWithProviders(<NotFound />);

    expect(screen.getByRole('heading', { name: 'Page not found' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Go to the dashboard' })).toHaveAttribute('href', '/');
  });
});
