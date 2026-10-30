/**
 * The signed-out header's Sign in control (ECO-744). On a phone a labelled "Sign in" button
 * wrapped to two lines and squeezed the search field, so below 600px it is an icon button.
 */

import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import Header from '../Header';

const signIn = vi.hoisted(() => vi.fn());

vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('../../../contexts/AuthContext', () => ({
  useAuth: () => ({
    user: null,
    isAuthenticated: false,
    isLoading: false,
    isConfigured: true,
    accountUrl: null,
    error: null,
    signIn,
    signOut: vi.fn(),
    clearError: vi.fn(),
  }),
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

describe('Header Sign in', () => {
  const originalMatchMedia = window.matchMedia;

  beforeEach(() => {
    signIn.mockReset();
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  test('on a phone, Sign in is an icon button with no visible label', async () => {
    setViewportWidth(390);
    render(<Header onMenuClick={vi.fn()} />);

    const button = screen.getByRole('button', { name: 'Sign in' });
    expect(button).not.toHaveTextContent(/sign in/i);
    expect(screen.queryByText(/^sign in$/i)).not.toBeInTheDocument();

    await userEvent.click(button);
    expect(signIn).toHaveBeenCalledTimes(1);
  });

  test('from 600px up, Sign in is a labelled button that does not wrap', async () => {
    setViewportWidth(600);
    render(<Header onMenuClick={vi.fn()} />);

    const button = screen.getByRole('button', { name: 'Sign in' });
    expect(button).toHaveTextContent('Sign in');
    expect(window.getComputedStyle(button).whiteSpace).toBe('nowrap');

    await userEvent.click(button);
    expect(signIn).toHaveBeenCalledTimes(1);
  });
});
