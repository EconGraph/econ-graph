import React from 'react';
import { render, screen, within } from '@testing-library/react';
import { Button } from '@mui/material';
import SeriesAnnotationsPanel from '../SeriesAnnotationsPanel';
import type { SeriesAnnotation } from '../../../hooks/useSeriesAnnotations';

const ANNOTATIONS: SeriesAnnotation[] = [
  {
    id: 'a',
    authorId: 'u1',
    date: '2020-04-01',
    title: 'Lockdown trough',
    description: null,
    visibility: 'PUBLIC',
  },
  {
    id: 'b',
    authorId: 'u1',
    date: '2020-01-01',
    title: 'Pre-pandemic peak',
    description: null,
    visibility: 'PUBLIC',
  },
];

describe('SeriesAnnotationsPanel', () => {
  test('shows extra controls in the header and under each annotation', () => {
    render(
      <SeriesAnnotationsPanel
        annotations={ANNOTATIONS}
        headerAction={<Button>Add annotation</Button>}
        renderAnnotationExtra={annotation => <span>{`Comments on ${annotation.id}`}</span>}
      />
    );

    expect(screen.getByRole('button', { name: 'Add annotation' })).toBeInTheDocument();
    const items = within(screen.getByRole('list', { name: 'Annotations' })).getAllByRole(
      'listitem'
    );
    expect(items).toHaveLength(2);
    expect(within(items[0]).getByText('Comments on a')).toBeInTheDocument();
    expect(within(items[1]).getByText('Comments on b')).toBeInTheDocument();
  });

  test('shows a loading placeholder while annotations load', () => {
    render(<SeriesAnnotationsPanel annotations={[]} isLoading />);

    expect(screen.getByTestId('annotations-loading')).toBeInTheDocument();
    expect(screen.queryByText(/No annotations/)).not.toBeInTheDocument();
  });
});
