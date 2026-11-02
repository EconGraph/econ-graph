import React from 'react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { auditPage } from './audit';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from '../../App';

describe('App accessibility', () => {
  it.each(['/about', '/privacy', '/missing-page'])(
    'audits the real app shell on %s',
    async path => {
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      const { unmount } = render(
        <QueryClientProvider client={client}>
          <MemoryRouter initialEntries={[path]}>
            <App />
          </MemoryRouter>
        </QueryClientProvider>
      );
      try {
        expect(await screen.findByRole('main')).toBeVisible();
        expect(screen.getAllByRole('heading').length).toBeGreaterThan(0);
        await auditPage(path.slice(1));
      } finally {
        unmount();
        client.clear();
      }
    }
  );
});
