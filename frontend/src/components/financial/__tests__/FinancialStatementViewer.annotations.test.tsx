/**
 * The viewer suspends while a statement's annotations load: showing them replaces the viewer
 * with its own loading fallback until they are in, loads them once, keeps them when they are hidden and shown again,
 * and sends a failed load to the nearest error boundary.
 */

import React, { Suspense } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FinancialStatementViewer } from '../FinancialStatementViewer';
import { ErrorBoundary } from '../../common/ErrorBoundary';
import { executeGraphQL } from '../../../utils/graphql';
import {
  GET_FINANCIAL_ANNOTATIONS,
  GET_FINANCIAL_STATEMENT,
} from '../../../test-utils/mocks/graphql/financial-queries';

vi.mock('../../../utils/graphql', () => ({ executeGraphQL: vi.fn() }));

const mockExecute = vi.mocked(executeGraphQL);

const ANNOTATIONS = [
  { id: 'a1', lineItemId: 'li-cash', content: 'Cash is up', type: 'comment' },
  { id: 'a2', lineItemId: 'li-equity', content: 'Check equity', type: 'comment' },
];

/** A promise and the functions that settle it. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

let annotationsLoad: ReturnType<typeof deferred<unknown>>;

const annotationRequests = () =>
  mockExecute.mock.calls.filter(([request]) => request.query === GET_FINANCIAL_ANNOTATIONS).length;

function renderViewer() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <ErrorBoundary fallback={<div>Error boundary caught it</div>}>
        <Suspense fallback={<div>Outer fallback</div>}>
          <FinancialStatementViewer companyId='company-1' statementId='statement-1' />
        </Suspense>
      </ErrorBoundary>
    </QueryClientProvider>
  );
}

describe('FinancialStatementViewer annotations', () => {
  beforeEach(() => {
    annotationsLoad = deferred<unknown>();
    mockExecute.mockReset();
    mockExecute.mockImplementation(async request => {
      if (request.query === GET_FINANCIAL_STATEMENT) return { data: { financialStatement: null } };
      if (request.query === GET_FINANCIAL_ANNOTATIONS) return annotationsLoad.promise as never;
      throw new Error(`Unexpected query: ${request.query}`);
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('suspends until the annotations load, loads them once, and keeps them when toggled', async () => {
    renderViewer();
    const toggle = await screen.findByRole('button', { name: 'Annotations (0)' });
    expect(annotationRequests()).toBe(0);

    fireEvent.click(toggle);
    expect(await screen.findByText('Loading financial statement...')).toBeVisible();

    await act(async () => {
      annotationsLoad.resolve({ data: { annotations: ANNOTATIONS } });
    });
    expect(await screen.findByRole('button', { name: 'Annotations (2)' })).toBeVisible();
    expect(screen.queryByText('Loading financial statement...')).not.toBeInTheDocument();
    // The viewer's useQuery and the suspending child share one request
    expect(annotationRequests()).toBe(1);

    // Hide them, then show them again: still there, with no new request
    fireEvent.click(screen.getByRole('button', { name: 'Annotations (2)' }));
    expect(screen.getByRole('button', { name: 'Annotations (2)' })).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'Annotations (2)' }));
    expect(screen.queryByText('Loading financial statement...')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Annotations (2)' })).toBeVisible();
    expect(annotationRequests()).toBe(1);
  });

  it('sends a failed annotations load to the error boundary', async () => {
    // React reports the caught error to the console
    vi.spyOn(console, 'error').mockImplementation(() => {});
    renderViewer();

    fireEvent.click(await screen.findByRole('button', { name: 'Annotations (0)' }));
    expect(await screen.findByText('Loading financial statement...')).toBeVisible();

    await act(async () => {
      annotationsLoad.reject(new Error('annotations backend down'));
    });
    expect(await screen.findByText('Error boundary caught it')).toBeInTheDocument();
    expect(annotationRequests()).toBe(1);
  });
});
