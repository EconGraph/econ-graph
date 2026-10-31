import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Link, MemoryRouter } from 'react-router-dom';
import { vi } from 'vitest';

import CanonicalLink from '../CanonicalLink';

// setupTests.vitest.ts stubs the router; canonical URLs need real navigation.
vi.unmock('react-router-dom');

describe('CanonicalLink', () => {
  afterEach(cleanup);

  it('identifies the current page without filters, callback state, or fragments', () => {
    render(
      <MemoryRouter initialEntries={['/series/gdp?code=private&state=secret#chart']}>
        <CanonicalLink />
      </MemoryRouter>
    );

    expect(document.head.querySelector('link[rel="canonical"]')).toHaveAttribute(
      'href',
      `${window.location.origin}/series/gdp`
    );
  });

  it('updates the single canonical link after navigation and removes it on unmount', () => {
    const { unmount } = render(
      <MemoryRouter initialEntries={['/series/gdp']}>
        <CanonicalLink />
        <Link to='/sources?source=bls#catalog'>Browse sources</Link>
      </MemoryRouter>
    );

    fireEvent.click(screen.getByRole('link', { name: 'Browse sources' }));

    expect(document.head.querySelectorAll('link[rel="canonical"]')).toHaveLength(1);
    expect(document.head.querySelector('link[rel="canonical"]')).toHaveAttribute(
      'href',
      `${window.location.origin}/sources`
    );
    unmount();
    expect(document.head.querySelector('link[rel="canonical"]')).toBeNull();
  });
});
