/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Verify that sign-out returns to the page it began on, not the dashboard (ECO-418).
 */

import React from 'react';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { vi } from 'vitest';

import ReturnAfterSignOut from '../ReturnAfterSignOut';

// setupTests.vitest.ts stubs the router; this test needs real navigation.
vi.unmock('react-router-dom');

const completeSignOut = vi.fn<() => Promise<string | null>>();

vi.mock('../../contexts/AuthContext', () => ({
  useAuth: () => ({ completeSignOut }),
}));

const renderAt = (path: string) =>
  render(
    <MemoryRouter initialEntries={[path]}>
      <ReturnAfterSignOut />
      <Routes>
        <Route path='/series/gdp' element={<p>GDP series page</p>} />
        <Route path='/' element={<p>Dashboard</p>} />
      </Routes>
    </MemoryRouter>
  );

describe('ReturnAfterSignOut', () => {
  beforeEach(() => {
    completeSignOut.mockReset();
  });

  it('goes back to the page sign-out began on', async () => {
    completeSignOut.mockResolvedValue('/series/gdp');
    renderAt('/?state=so-1');

    expect(await screen.findByText('GDP series page')).toBeInTheDocument();
  });

  it('stays put when the page was not reached by signing out', async () => {
    completeSignOut.mockResolvedValue(null);
    renderAt('/');

    expect(await screen.findByText('Dashboard')).toBeInTheDocument();
    expect(completeSignOut).toHaveBeenCalled();
  });
});
