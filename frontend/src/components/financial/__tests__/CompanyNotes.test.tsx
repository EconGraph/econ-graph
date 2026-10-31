import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { vi } from 'vitest';
import CompanyNotes from '../CompanyNotes';
import { useAuth } from '../../../contexts/AuthContext';
import * as operations from '../../../graphql/companyNotes';

vi.mock('../../../contexts/AuthContext', () => ({ useAuth: vi.fn() }));
vi.mock('../../../graphql/companyNotes', async () => {
  const actual = await vi.importActual<typeof import('../../../graphql/companyNotes')>(
    '../../../graphql/companyNotes'
  );
  return {
    ...actual,
    getCompanyNotes: vi.fn(),
    createCompanyNote: vi.fn(),
    updateCompanyNote: vi.fn(),
    deleteCompanyNote: vi.fn(),
  };
});
const CIK = '0000320193';
const note: operations.CompanyNote = {
  id: 'note-alice', cik: CIK, anchor: operations.companyNoteAnchor(CIK),
  content: 'Original research', createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-02T00:00:00Z', targetStatus: 'found',
};
function auth(id: string | null) {
  vi.mocked(useAuth).mockReturnValue({ user: id ? { id, name: id } : null, isAuthenticated: !!id } as ReturnType<typeof useAuth>);
}
function fixture() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return {
    client,
    wrapper: ({ children }: { children: React.ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  };
}
beforeEach(() => {
  vi.clearAllMocks();
  auth('alice');
  vi.mocked(operations.getCompanyNotes).mockResolvedValue([note]);
  vi.mocked(operations.createCompanyNote).mockResolvedValue(note);
  vi.mocked(operations.updateCompanyNote).mockResolvedValue(note);
  vi.mocked(operations.deleteCompanyNote).mockResolvedValue(true);
});
it('does not fetch notes when signed out', () => {
  auth(null);
  render(<CompanyNotes cik={CIK} />);
  expect(screen.getByText('Sign in to keep private company notes.')).toBeInTheDocument();
  expect(operations.getCompanyNotes).not.toHaveBeenCalled();
});
it('creates company notes and edits/deletes only IDs supplied by its own listing', async () => {
  const user = userEvent.setup();
  const { wrapper } = fixture();
  render(<CompanyNotes cik={CIK} />, { wrapper });
  await screen.findByText('Original research');
  await user.type(screen.getByLabelText('New private note'), 'New evidence');
  await user.click(screen.getByRole('button', { name: 'Save note' }));
  await waitFor(() => expect(operations.createCompanyNote).toHaveBeenCalledWith(operations.companyNoteAnchor(CIK), 'New evidence'));
  await user.click(screen.getByRole('button', { name: 'Edit note' }));
  await user.clear(screen.getByLabelText('Edit private note'));
  await user.type(screen.getByLabelText('Edit private note'), 'Revised evidence');
  await user.click(screen.getByRole('button', { name: 'Save changes' }));
  await waitFor(() => expect(operations.updateCompanyNote).toHaveBeenCalledWith('note-alice', 'Revised evidence'));
  await user.click(screen.getByRole('button', { name: 'Delete note' }));
  await waitFor(() => expect(operations.deleteCompanyNote).toHaveBeenCalled());
  expect(vi.mocked(operations.deleteCompanyNote).mock.calls[0][0]).toBe('note-alice');
});
it('clears prior account content and drafts when the account changes', async () => {
  const user = userEvent.setup();
  const { wrapper } = fixture();
  const { rerender } = render(<CompanyNotes cik={CIK} />, { wrapper });
  await screen.findByText('Original research');
  await user.click(screen.getByRole('button', { name: 'Edit note' }));
  auth('bob');
  vi.mocked(operations.getCompanyNotes).mockResolvedValue([]);
  rerender(<CompanyNotes cik={CIK} />);
  expect(screen.queryByText('Original research')).not.toBeInTheDocument();
  expect(screen.queryByLabelText('Edit private note')).not.toBeInTheDocument();
  expect(screen.getByLabelText('New private note')).toHaveValue('');
  await screen.findByText('No notes for this target yet.');
});
it('preserves note content and explains unavailable source targets', async () => {
  vi.mocked(operations.getCompanyNotes).mockResolvedValue([{ ...note, targetStatus: 'unavailable' }]);
  render(<CompanyNotes cik={CIK} />, { wrapper: fixture().wrapper });
  await screen.findByText('Original research');
  expect(screen.getByText('Original target is currently unavailable. Your note is preserved.')).toBeInTheDocument();
});
it('uses the selected immutable anchor for a new fact note', async () => {
  const user = userEvent.setup();
  const anchor = JSON.stringify({ version: '1', target: { kind: 'filed_fact', accession: 'immutable' } });
  vi.mocked(operations.getCompanyNotes).mockResolvedValue([]);
  render(<CompanyNotes cik={CIK} anchor={anchor} targetLabel="Original filing" />, { wrapper: fixture().wrapper });
  await screen.findByText('No notes for this target yet.');
  await user.type(screen.getByLabelText('New private note'), 'Pinned evidence');
  await user.click(screen.getByRole('button', { name: 'Save note' }));
  await waitFor(() => expect(operations.createCompanyNote).toHaveBeenCalledWith(anchor, 'Pinned evidence'));
});
