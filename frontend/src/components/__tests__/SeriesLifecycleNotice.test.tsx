import React from 'react';
import { screen } from '@testing-library/react';
import { renderWithProviders } from '../../test-utils/test-providers';
import SeriesLifecycleNotice from '../SeriesLifecycleNotice';
import type { SeriesLifecycle } from '../../utils/graphql';

const missing: SeriesLifecycle = {
  status: 'missing_from_catalog',
  since: '2026-10-25T00:15:00Z',
  successorId: null,
  proposedSuccessorId: null,
};

describe('SeriesLifecycleNotice', () => {
  it('uses the confirmation date and keeps the source name in the notice', () => {
    renderWithProviders(<SeriesLifecycleNotice lifecycle={missing} sourceName='FRED' />);
    expect(screen.getByRole('alert')).toHaveTextContent(
      'No longer published by FRED since 2026-10-25'
    );
    expect(screen.getByRole('alert')).toHaveTextContent('Stored history remains available');
  });

  it('does not claim that an unconfirmed successor replaces the series', () => {
    renderWithProviders(
      <SeriesLifecycleNotice lifecycle={{ ...missing, proposedSuccessorId: 'candidate' }} />
    );
    expect(
      screen.getByRole('link', { name: 'Possible successor (not confirmed)' })
    ).toHaveAttribute('href', '/series/candidate');
    expect(screen.queryByText(/Superseded by/)).not.toBeInTheDocument();
  });

  it('links the confirmed successor without displaying an old candidate', () => {
    renderWithProviders(
      <SeriesLifecycleNotice
        lifecycle={{
          ...missing,
          status: 'superseded',
          successorId: 'confirmed',
          proposedSuccessorId: 'candidate',
        }}
      />
    );
    expect(screen.getByRole('link', { name: 'Superseded by another series' })).toHaveAttribute(
      'href',
      '/series/confirmed'
    );
    expect(screen.queryByText(/Possible successor/)).not.toBeInTheDocument();
  });

  it('does not manufacture a date when the API has none', () => {
    renderWithProviders(<SeriesLifecycleNotice lifecycle={{ ...missing, since: null }} compact />);
    expect(screen.getByText('No longer published by the source')).toBeInTheDocument();
    expect(screen.queryByText(/since/)).not.toBeInTheDocument();
  });

  it('does not show a disappearance notice for active series', () => {
    renderWithProviders(<SeriesLifecycleNotice lifecycle={{ ...missing, status: 'active' }} />);
    expect(screen.queryByText(/No longer published/)).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
