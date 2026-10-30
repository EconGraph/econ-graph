/**
 * The navigation drawer links to the Privacy Policy (ECO-730). Below 600px the header's Privacy
 * button is hidden, so on a phone the drawer is the only way to reach /privacy by clicking.
 */

import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import Sidebar from '../Sidebar';

const router = vi.hoisted(() => ({ navigate: vi.fn(), pathname: '/' }));

vi.mock('react-router-dom', () => ({
  useNavigate: () => router.navigate,
  useLocation: () => ({ pathname: router.pathname, search: '', hash: '', state: null }),
}));

/**
 * Make window.matchMedia answer width queries as a viewport of the given width would.
 * @param width - The viewport width in px.
 */
function setViewportWidth(width: number) {
  window.matchMedia = ((query: string) => {
    const max = /max-width:\s*([\d.]+)px/.exec(query);
    const min = /min-width:\s*([\d.]+)px/.exec(query);
    const matches = (!max || width <= Number(max[1])) && (!min || width >= Number(min[1]));
    return {
      matches,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    } as unknown as ReturnType<typeof window.matchMedia>;
  }) as typeof window.matchMedia;
}

describe('Sidebar privacy link', () => {
  const originalMatchMedia = window.matchMedia;

  beforeEach(() => {
    router.navigate.mockReset();
    router.pathname = '/';
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  test('on a phone, the drawer has a Privacy Policy item that goes to /privacy and closes it', async () => {
    setViewportWidth(393); // Pixel 5
    const onClose = vi.fn();
    render(<Sidebar open onClose={onClose} />);

    await userEvent.click(screen.getByRole('button', { name: /^Navigate to Privacy Policy/ }));

    expect(router.navigate).toHaveBeenCalledWith('/privacy');
    expect(onClose).toHaveBeenCalled();
  });

  test('marks the Privacy Policy item as the current page on /privacy', () => {
    setViewportWidth(393);
    router.pathname = '/privacy';
    render(<Sidebar open onClose={vi.fn()} />);

    expect(screen.getByRole('button', { name: /^Navigate to Privacy Policy/ })).toHaveAttribute(
      'aria-current',
      'page'
    );
  });
});

describe('Sidebar sources', () => {
  // ECO-751: no item singles out one source; Data Sources lists them all
  test('has Data Sources and no FRED Data item', () => {
    render(<Sidebar open onClose={vi.fn()} />);

    expect(screen.getAllByRole('button', { name: /^Navigate to Data Sources/ }).length).toBeGreaterThan(0);
    expect(screen.queryAllByRole('button', { name: /Navigate to FRED Data/ })).toHaveLength(0);
  });
});
