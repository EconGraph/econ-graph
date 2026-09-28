/**
 * Signed-in annotation editing on the series page, against GraphQL response fixtures in the
 * backend's shape.
 *
 * The real hooks run; only executeGraphQL and the auth context are replaced, so these tests
 * check the mutations and variables the page sends as well as what each viewer sees.
 */

import React from 'react';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import { useParams } from 'react-router-dom';
import { TestProviders } from '../../test-utils/test-providers';
import { useAuth } from '../../contexts/AuthContext';
import { executeGraphQL, GraphQLRequest, MUTATIONS, QUERIES } from '../../utils/graphql';
import SeriesDetail from '../SeriesDetail';
import {
  ALICE,
  BOB,
  GDP_ID,
  gdpAnnotations,
  gdpAnnotationsForAlice,
  gdpDetail,
  gdpLevels,
  lockdownComments,
} from './fixtures/seriesAnnotations';

// setupTests mocks these hooks globally; this page is tested with the real ones.
vi.mock('../../hooks/useSeriesData', async () => vi.importActual('../../hooks/useSeriesData'));
vi.mock('../../hooks/useSeriesAnnotations', async () =>
  vi.importActual('../../hooks/useSeriesAnnotations')
);

vi.mock('../../utils/graphql', async () => {
  const actual = await vi.importActual<typeof import('../../utils/graphql')>(
    '../../utils/graphql'
  );
  return { ...actual, executeGraphQL: vi.fn() };
});

vi.mock('react-router-dom', () => ({
  useParams: vi.fn(),
  useNavigate: () => vi.fn(),
}));

// Chart.js needs a canvas. Render each observation and annotation as a button instead. As in
// Chart.js, the annotation plugin sees a click event before the chart's own onClick, which gets
// the same event and the point nearest the click (here, the first one).
vi.mock('react-chartjs-2', () => ({
  Line: ({ data, options }: any) => {
    const drawn: Record<string, any> = options.plugins.annotation.annotations;
    return (
      <div data-testid='line-chart'>
        {data.datasets[0].data.map((p: any, index: number) => (
          <button
            key={p.date}
            type='button'
            data-testid='chart-point'
            onClick={() => options.onClick?.({ type: 'click' }, [{ datasetIndex: 0, index }])}
          >
            {p.date}
          </button>
        ))}
        {Object.entries(drawn).map(([id, a]) => (
          <button
            key={id}
            type='button'
            data-testid='chart-annotation'
            onClick={() => {
              const event = { type: 'click' };
              a.click?.({}, event);
              options.onClick?.(event, [{ datasetIndex: 0, index: 0 }]);
            }}
          >
            {a.label.content}
          </button>
        ))}
      </div>
    );
  },
}));

const mockExecute = vi.mocked(executeGraphQL);
const mockUseParams = vi.mocked(useParams);
const mockUseAuth = vi.mocked(useAuth);

type Viewer = 'alice' | 'bob' | 'signed-out';

/**
 * Sign in as a viewer, or sign out, with sign-in configured.
 * @param viewer - Who is looking at the page.
 * @returns The mocked `signIn`.
 */
function viewAs(viewer: Viewer) {
  const signIn = vi.fn();
  const user =
    viewer === 'alice'
      ? { id: ALICE, name: 'Alice' }
      : viewer === 'bob'
        ? { id: BOB, name: 'Bob' }
        : null;
  mockUseAuth.mockReturnValue({
    user,
    isAuthenticated: user !== null,
    isLoading: false,
    isConfigured: true,
    accountUrl: null,
    error: null,
    signIn,
    signOut: vi.fn(),
    completeSignIn: vi.fn(),
    clearError: vi.fn(),
  });
  return signIn;
}

/**
 * Answer GraphQL requests from fixtures, as the backend would for this viewer.
 * @param viewer - Who is signed in; only Alice is sent her private annotation.
 * @param overrides - Responses (or errors) for specific operations.
 */
function serve(viewer: Viewer, overrides: Partial<Record<string, unknown>> = {}) {
  mockExecute.mockImplementation(async (request: GraphQLRequest) => {
    if (request.query in overrides) {
      const answer = overrides[request.query];
      if (answer instanceof Error) throw answer;
      return { data: answer };
    }
    switch (request.query) {
      case QUERIES.GET_SERIES_DETAIL:
        return { data: gdpDetail };
      case QUERIES.GET_SERIES_DATA:
        return { data: gdpLevels };
      case QUERIES.GET_ANNOTATIONS_FOR_SERIES:
        return { data: viewer === 'alice' ? gdpAnnotationsForAlice : gdpAnnotations };
      case QUERIES.GET_COMMENTS_FOR_ANNOTATION:
        return { data: lockdownComments };
      case MUTATIONS.CREATE_ANNOTATION: {
        const input = request.variables?.input;
        return {
          data: {
            createAnnotation: {
              id: 'a1d6c0e2-5555-4c8e-9a51-0d3f2b7e8c05',
              userId: ALICE,
              annotationDate: input.annotationDate,
              title: input.title,
              description: input.content,
              color: null,
              visibility: input.isPublic ? 'PUBLIC' : 'PRIVATE',
            },
          },
        };
      }
      case MUTATIONS.UPDATE_ANNOTATION:
        return { data: { updateAnnotation: gdpAnnotations.annotationsForSeries[0] } };
      case MUTATIONS.DELETE_ANNOTATION:
        return { data: { deleteAnnotation: true } };
      case MUTATIONS.ADD_COMMENT:
        return {
          data: {
            addComment: {
              id: 'c0de0000-0003-4000-8000-000000000003',
              annotationId: request.variables?.input.annotationId,
              userId: ALICE,
              content: request.variables?.input.content,
              createdAt: '2020-08-03T09:00:00Z',
            },
          },
        };
      default:
        throw new Error('unexpected query');
    }
  });
}

function renderPage() {
  mockUseParams.mockReturnValue({ id: GDP_ID });
  return render(
    <TestProviders>
      <SeriesDetail />
    </TestProviders>
  );
}

const requestsFor = (query: string) =>
  mockExecute.mock.calls.map(([request]) => request).filter(request => request.query === query);

/** The list item of the annotation with this title. */
const row = (title: string) =>
  within(screen.getByRole('list', { name: 'Annotations' }))
    .getAllByRole('listitem')
    .find(item => within(item).queryByText(title))!;

const loaded = () => screen.findByRole('list', { name: 'Annotations' });

describe('SeriesDetail annotation editing', () => {
  beforeEach(() => {
    mockExecute.mockReset();
  });

  test('shows the private badge and edit controls only on the author’s annotations', async () => {
    viewAs('alice');
    serve('alice');
    renderPage();
    await loaded();

    const privateRow = row('My draft note');
    expect(within(privateRow).getByTestId('private-badge')).toHaveTextContent('Private');
    expect(screen.getAllByTestId('private-badge')).toHaveLength(1);

    // Alice wrote the lockdown and pre-pandemic notes; Bob wrote the rebound.
    for (const title of ['My draft note', 'Lockdown trough', 'Pre-pandemic peak']) {
      expect(within(row(title)).getByRole('button', { name: `Edit ${title}` })).toBeVisible();
      expect(within(row(title)).getByRole('button', { name: `Delete ${title}` })).toBeVisible();
    }
    expect(within(row('Reopening rebound')).queryByRole('button', { name: /^edit /i })).toBeNull();
    expect(
      within(row('Reopening rebound')).queryByRole('button', { name: /^delete /i })
    ).toBeNull();
  });

  test('another signed-in user gets no edit controls on others’ public annotations', async () => {
    viewAs('bob');
    serve('bob');
    renderPage();
    await loaded();

    expect(screen.queryByText('My draft note')).not.toBeInTheDocument();
    expect(screen.queryByTestId('private-badge')).not.toBeInTheDocument();
    expect(within(row('Lockdown trough')).queryByRole('button', { name: /^edit /i })).toBeNull();
    expect(
      within(row('Reopening rebound')).getByRole('button', { name: 'Edit Reopening rebound' })
    ).toBeVisible();
  });

  test('never offers badge or edit controls on someone else’s private annotation', async () => {
    // The backend only sends a private annotation to its author; check the page doesn't rely
    // on that alone.
    viewAs('bob');
    serve('bob', { [QUERIES.GET_ANNOTATIONS_FOR_SERIES]: gdpAnnotationsForAlice });
    renderPage();
    await loaded();

    const privateRow = row('My draft note');
    expect(within(privateRow).queryByTestId('private-badge')).toBeNull();
    expect(within(privateRow).queryByRole('button', { name: /^edit /i })).toBeNull();
    expect(within(privateRow).queryByRole('button', { name: /^delete /i })).toBeNull();
  });

  test('signed-out visitors see no edit controls, only a way to sign in', async () => {
    const signIn = viewAs('signed-out');
    serve('signed-out');
    renderPage();
    await loaded();
    const user = userEvent.setup();

    expect(screen.queryByRole('button', { name: /add annotation/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^edit /i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^delete /i })).not.toBeInTheDocument();
    expect(screen.queryByTestId('private-badge')).not.toBeInTheDocument();

    // Clicking the chart doesn't offer to annotate.
    await user.click(screen.getAllByTestId('chart-point')[1]);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    // Public comments can be read, but not added.
    await user.click(within(row('Lockdown trough')).getByRole('button', {
        name: 'Comments on Lockdown trough',
      }));
    expect(await screen.findByText('Services drove most of it.')).toBeInTheDocument();
    expect(screen.queryByLabelText(/^Add a comment/)).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Sign in to annotate' }));
    expect(signIn).toHaveBeenCalled();
  });

  test('creates a private annotation by default from the header button', async () => {
    viewAs('alice');
    serve('alice');
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Add annotation' }));
    const dialog = screen.getByRole('dialog', { name: 'Add annotation' });
    // Opens on the latest observation.
    expect(within(dialog).getByLabelText(/^Date/)).toHaveValue('2020-07-01');
    expect(within(dialog).getByRole('checkbox', { name: 'Public' })).not.toBeChecked();

    await user.type(within(dialog).getByLabelText(/^Title/), '  Q3 bounce  ');
    await user.type(within(dialog).getByLabelText('Note'), 'Stimulus.');
    const annotationLoads = requestsFor(QUERIES.GET_ANNOTATIONS_FOR_SERIES).length;
    await user.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    const [create] = requestsFor(MUTATIONS.CREATE_ANNOTATION);
    // No author in the request: the backend takes it from the token.
    expect(create.variables).toEqual({
      input: {
        seriesId: GDP_ID,
        annotationDate: '2020-07-01',
        title: 'Q3 bounce',
        content: 'Stimulus.',
        annotationType: 'note',
        isPublic: false,
      },
    });
    expect(MUTATIONS.CREATE_ANNOTATION).toMatch(/\bvisibility\b/);
    // The list reloads with the new annotation.
    await waitFor(() =>
      expect(requestsFor(QUERIES.GET_ANNOTATIONS_FOR_SERIES).length).toBeGreaterThan(
        annotationLoads
      )
    );
  });

  test('creates a public annotation on the point clicked on the chart', async () => {
    viewAs('alice');
    serve('alice');
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: '2020-04-01' }));
    const dialog = screen.getByRole('dialog', { name: 'Add annotation' });
    expect(within(dialog).getByLabelText(/^Date/)).toHaveValue('2020-04-01');

    await user.type(within(dialog).getByLabelText(/^Title/), 'Trough');
    await user.click(within(dialog).getByRole('checkbox', { name: 'Public' }));
    expect(within(dialog).getByText(/Everyone who opens this series/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(requestsFor(MUTATIONS.CREATE_ANNOTATION)).toHaveLength(1));
    expect(requestsFor(MUTATIONS.CREATE_ANNOTATION)[0].variables?.input).toMatchObject({
      annotationDate: '2020-04-01',
      title: 'Trough',
      isPublic: true,
    });
  });

  test('clicking an annotation on the chart selects it rather than starting a new one', async () => {
    viewAs('alice');
    serve('alice');
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(
      within(screen.getByTestId('line-chart')).getByRole('button', { name: 'Lockdown trough' })
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(within(row('Lockdown trough')).getAllByRole('button')[0]).toHaveAttribute(
      'aria-current',
      'true'
    );
  });

  test('shows why a save failed and keeps the form open', async () => {
    viewAs('alice');
    serve('alice', { [MUTATIONS.CREATE_ANNOTATION]: new Error('Insufficient permissions') });
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Add annotation' }));
    const dialog = screen.getByRole('dialog', { name: 'Add annotation' });
    await user.type(within(dialog).getByLabelText(/^Title/), 'Note');
    await user.click(within(dialog).getByRole('button', { name: 'Save' }));

    expect(await within(dialog).findByText('Insufficient permissions')).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'Add annotation' })).toBeInTheDocument();
  });

  test('edits an annotation, making it public', async () => {
    viewAs('alice');
    serve('alice');
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Edit My draft note' }));
    const dialog = screen.getByRole('dialog', { name: 'Edit annotation' });
    expect(within(dialog).getByLabelText(/^Date/)).toBeDisabled();
    expect(within(dialog).getByLabelText(/^Title/)).toHaveValue('My draft note');
    expect(within(dialog).getByRole('checkbox', { name: 'Public' })).not.toBeChecked();

    await user.clear(within(dialog).getByLabelText(/^Title/));
    await user.type(within(dialog).getByLabelText(/^Title/), 'Revision check');
    await user.click(within(dialog).getByRole('checkbox', { name: 'Public' }));
    await user.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(requestsFor(MUTATIONS.UPDATE_ANNOTATION)[0].variables).toEqual({
      input: {
        annotationId: 'a1d6c0e2-4444-4c8e-9a51-0d3f2b7e8c04',
        title: 'Revision check',
        content: 'Check the revision.',
        isPublic: true,
      },
    });
  });

  test('deletes an annotation after confirming', async () => {
    viewAs('alice');
    serve('alice');
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Delete Lockdown trough' }));
    const dialog = screen.getByRole('dialog', { name: 'Delete annotation?' });
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(requestsFor(MUTATIONS.DELETE_ANNOTATION)[0].variables).toEqual({
      input: { annotationId: 'a1d6c0e2-1111-4c8e-9a51-0d3f2b7e8c01' },
    });
  });

  test('keeps the delete dialog open when the backend deletes nothing', async () => {
    viewAs('alice');
    serve('alice', { [MUTATIONS.DELETE_ANNOTATION]: { deleteAnnotation: false } });
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(screen.getByRole('button', { name: 'Delete Lockdown trough' }));
    const dialog = screen.getByRole('dialog', { name: 'Delete annotation?' });
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }));

    expect(await within(dialog).findByText('Could not delete the annotation.')).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'Delete annotation?' })).toBeInTheDocument();
  });

  test('shows the comment thread and posts a comment', async () => {
    viewAs('alice');
    serve('alice');
    renderPage();
    await loaded();
    const user = userEvent.setup();

    await user.click(within(row('Lockdown trough')).getByRole('button', {
        name: 'Comments on Lockdown trough',
      }));
    const thread = await screen.findByRole('list', { name: 'Comments on Lockdown trough' });
    expect(
      within(row('Lockdown trough')).getByRole('button', { name: 'Hide comments on Lockdown trough' })
    ).toHaveAttribute('aria-expanded', 'true');
    const comments = within(thread).getAllByRole('listitem');
    // Oldest first; Alice's own comment is labelled as hers.
    expect(comments[0]).toHaveTextContent('Services drove most of it.');
    expect(comments[0]).toHaveTextContent('Another user');
    expect(comments[1]).toHaveTextContent('Revised down in the annual update.');
    expect(comments[1]).toHaveTextContent('You');
    expect(comments[1]).not.toHaveTextContent('Another user');
    expect(requestsFor(QUERIES.GET_COMMENTS_FOR_ANNOTATION)[0].variables).toEqual({
      annotationId: 'a1d6c0e2-1111-4c8e-9a51-0d3f2b7e8c01',
    });

    await user.type(screen.getByLabelText('Add a comment on Lockdown trough'), 'Agreed.');
    await user.click(screen.getByRole('button', { name: 'Post comment on Lockdown trough' }));

    await waitFor(() =>
      expect(screen.getByLabelText('Add a comment on Lockdown trough')).toHaveValue('')
    );
    expect(requestsFor(MUTATIONS.ADD_COMMENT)[0].variables).toEqual({
      input: { annotationId: 'a1d6c0e2-1111-4c8e-9a51-0d3f2b7e8c01', content: 'Agreed.' },
    });
    // The thread reloads after posting.
    expect(requestsFor(QUERIES.GET_COMMENTS_FOR_ANNOTATION).length).toBeGreaterThan(1);
  });
});
