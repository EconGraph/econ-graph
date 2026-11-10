/**
 * ECO-881: every page's footer links to the Privacy Policy and the Terms of Use.
 */

import React from 'react';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import SiteFooter from '../SiteFooter';

vi.unmock('react-router-dom');

describe('SiteFooter', () => {
  it('links to the Privacy Policy and the Terms of Use', () => {
    render(
      <MemoryRouter>
        <SiteFooter />
      </MemoryRouter>
    );

    const footer = screen.getByRole('contentinfo');
    expect(within(footer).getByRole('link', { name: 'Privacy Policy' })).toHaveAttribute(
      'href',
      '/privacy'
    );
    expect(within(footer).getByRole('link', { name: 'Terms of Use' })).toHaveAttribute(
      'href',
      '/terms'
    );
  });

  it('the Terms of Use link navigates to /terms', async () => {
    render(
      <MemoryRouter initialEntries={['/explore']}>
        <Routes>
          <Route path='/terms' element={<p>terms page</p>} />
          <Route path='*' element={null} />
        </Routes>
        <SiteFooter />
      </MemoryRouter>
    );

    await userEvent.click(screen.getByRole('link', { name: 'Terms of Use' }));
    expect(screen.getByText('terms page')).toBeInTheDocument();
  });
});
