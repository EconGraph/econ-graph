/**
 * Regression test for ECO-335. On a phone the sidebar is a modal drawer, which covers the page
 * and marks the rest of the app aria-hidden while open. It used to start open at every width, so
 * on a phone the whole app was hidden from assistive tech and its controls (such as the world map's
 * Projection select) were unreachable until the drawer was dismissed.
 */

import React from 'react';
import { render, screen } from '@testing-library/react';
import { vi } from 'vitest';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from '../App';

vi.mock('../contexts/AuthContext', () => ({
  AuthProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useAuth: () => ({
    user: null,
    isAuthenticated: false,
    isLoading: false,
    isConfigured: false,
    accountUrl: null,
    error: null,
    signIn: vi.fn(),
    signOut: vi.fn(),
    completeSignIn: vi.fn(),
    clearError: vi.fn(),
  }),
}));

// Not under test, and heavy to import.
vi.mock('../pages/Dashboard', () => ({ default: () => null }));
vi.mock('../pages/SeriesExplorer', () => ({ default: () => null }));
vi.mock('../pages/SeriesDetail', () => ({ default: () => null }));
vi.mock('../pages/DataSources', () => ({ default: () => null }));
vi.mock('../pages/About', () => ({ default: () => null }));
vi.mock('../pages/GlobalAnalysis', () => ({ default: () => null }));
vi.mock('../pages/PrivacyPolicy', () => ({ default: () => null }));
vi.mock('../pages/NotFound', () => ({ default: () => null }));
vi.mock('../pages/AuthCallback', () => ({ default: () => null }));

/**
 * Make window.matchMedia answer width queries as a viewport of the given width would.
 * @param width - The viewport width in px.
 */
function setViewportWidth(width: number) {
  window.matchMedia = ((query: string) => {
    const max = /max-width:\s*([\d.]+)px/.exec(query);
    const min = /min-width:\s*([\d.]+)px/.exec(query);
    const matches =
      (!max || width <= Number(max[1])) && (!min || width >= Number(min[1]));
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

function renderApp() {
  return render(
    <QueryClientProvider client={new QueryClient()}>
      <App />
    </QueryClientProvider>
  );
}

describe('App sidebar', () => {
  const originalMatchMedia = window.matchMedia;

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  test('starts closed on a phone, leaving the page reachable', () => {
    setViewportWidth(393); // Pixel 5

    const { container } = renderApp();

    // Nothing in the app is hidden from assistive tech, and the page content is reachable.
    // (getByRole skips anything inside an aria-hidden ancestor.)
    // An open MUI modal sets aria-hidden on its siblings under <body>, i.e. this container.
    expect(container).not.toHaveAttribute('aria-hidden', 'true');
    expect(screen.getByRole('main')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'open drawer' })).toBeInTheDocument();
    // The drawer is mounted (keepMounted) but not shown.
    expect(screen.queryByTestId('sidebar-nav-global-analysis')).not.toBeVisible();
  });

  test('starts open on a desktop', () => {
    setViewportWidth(1280);

    renderApp();

    expect(screen.getByTestId('sidebar-desktop')).toBeInTheDocument();
    expect(screen.getByRole('main')).toBeInTheDocument();
  });
});
