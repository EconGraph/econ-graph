/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Verify the callback route returns to the saved page, or shows the error with a way home.
 */

import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { vi } from 'vitest';

import AuthCallback from '../AuthCallback';

// setupTests.vitest.ts stubs the router; this test needs real navigation.
vi.unmock('react-router-dom');

const completeSignIn = vi.fn<() => Promise<string>>();

vi.mock('../../contexts/AuthContext', () => ({
  useAuth: () => ({ completeSignIn }),
}));

const renderAt = (path: string) =>
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path='/auth/callback' element={<AuthCallback />} />
        <Route path='/series/gdp' element={<p>GDP series page</p>} />
        <Route path='/' element={<p>Dashboard</p>} />
      </Routes>
    </MemoryRouter>
  );

describe('AuthCallback', () => {
  beforeEach(() => {
    completeSignIn.mockReset();
  });

  it('shows progress, then returns to the page sign-in began on', async () => {
    completeSignIn.mockResolvedValue('/series/gdp');
    renderAt('/auth/callback?code=abc&state=xyz');

    expect(screen.getByText(/signing you in/i)).toBeInTheDocument();
    expect(await screen.findByText('GDP series page')).toBeInTheDocument();
  });

  it('shows why sign-in failed and links back to the dashboard', async () => {
    completeSignIn.mockRejectedValue(new Error('No matching state found in storage'));
    renderAt('/auth/callback?code=abc&state=stale');

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Sign-in did not complete: No matching state found in storage'
    );
    await userEvent.click(screen.getByRole('button', { name: /back to the dashboard/i }));
    expect(await screen.findByText('Dashboard')).toBeInTheDocument();
  });
});
