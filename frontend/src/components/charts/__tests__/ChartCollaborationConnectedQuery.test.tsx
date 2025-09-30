import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient } from '@tanstack/react-query';
import { vi } from 'vitest';
import { ChartCollaborationConnectedQuery } from '../ChartCollaborationConnectedQuery';
import { server } from '../../../test-utils/mocks/server';
import { renderWithProviders } from '../../../test-utils/test-providers';

// Mock the auth context
const mockUser = {
  id: 'user-1',
  name: 'Test User',
  email: 'test@example.com',
};

vi.mock('../../../contexts/AuthContext', () => ({
  useAuth: () => ({ user: mockUser }),
  AuthProvider: ({ children }: { children: React.ReactNode }) => children,
}));

// Mock the GraphQL execution
vi.mock('../../../utils/graphql', () => ({
  executeGraphQL: vi.fn(),
  QUERIES: {
    GET_CHART_COLLABORATORS: 'query GetChartCollaborators($chartId: ID!) { chartCollaborators(chartId: $chartId) { id user_id role last_accessed_at } }',
    GET_USER: 'query GetUser($userId: ID!) { user(userId: $userId) { id name email avatarUrl } }',
    GET_ANNOTATIONS: 'query GetAnnotations($chartId: ID!) { annotationsForChart(chartId: $chartId) { id title description content user_id created_at is_pinned is_visible } }',
    GET_COMMENTS_FOR_ANNOTATION: 'query GetCommentsForAnnotation($annotationId: ID!) { commentsForAnnotation(annotationId: $annotationId) { id content user_id created_at } }',
  },
}));

import { executeGraphQL } from '../../../utils/graphql';

const mockExecuteGraphQL = executeGraphQL as any;

describe('ChartCollaborationConnectedQuery', () => {
  let queryClient: QueryClient;

  beforeEach(() => {
    queryClient = new QueryClient({
      defaultOptions: {
        queries: {
          retry: false,
          cacheTime: 0,
          staleTime: 0,
        },
        mutations: {
          retry: false,
        },
      },
    });

    // Mock GraphQL responses
    mockExecuteGraphQL.mockImplementation(async ({ query, variables }) => {
      if (query.includes('GetChartCollaborators')) {
        return {
          data: {
            chartCollaborators: [
              {
                id: 'collab-1',
                user_id: 'user-1',
                role: 'owner',
                last_accessed_at: new Date().toISOString(),
              },
              {
                id: 'collab-2',
                user_id: 'user-2',
                role: 'viewer',
                last_accessed_at: new Date(Date.now() - 60000).toISOString(), // 1 minute ago
              },
            ],
          },
        };
      }

      if (query.includes('GetUser')) {
        const userId = variables?.userId;
        const users = {
          'user-1': {
            id: 'user-1',
            name: 'Test User',
            email: 'test@example.com',
            avatarUrl: 'https://example.com/avatar1.jpg',
          },
          'user-2': {
            id: 'user-2',
            name: 'Another User',
            email: 'another@example.com',
            avatarUrl: 'https://example.com/avatar2.jpg',
          },
        };
        return {
          data: {
            user: users[userId as keyof typeof users] || null,
          },
        };
      }

      if (query.includes('GetAnnotations')) {
        return {
          data: {
            annotationsForChart: [
              {
                id: 'annotation-1',
                title: 'GDP Growth Analysis',
                description: 'Analysis of GDP growth trends',
                content: 'This shows the GDP growth over time',
                user_id: 'user-1',
                created_at: new Date().toISOString(),
                is_pinned: false,
                is_visible: true,
                type: 'analysis',
              },
              {
                id: 'annotation-2',
                title: 'Market Trends',
                description: 'Key market trends to watch',
                content: 'Important market indicators',
                user_id: 'user-2',
                created_at: new Date().toISOString(),
                is_pinned: true,
                is_visible: true,
                type: 'forecast',
              },
            ],
          },
        };
      }

      if (query.includes('GetCommentsForAnnotation')) {
        return {
          data: {
            commentsForAnnotation: [
              {
                id: 'comment-1',
                content: 'Great analysis!',
                user_id: 'user-2',
                created_at: new Date().toISOString(),
              },
            ],
          },
        };
      }

      return { data: null };
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('should render collaboration panel with data', async () => {
    renderWithProviders(
      <ChartCollaborationConnectedQuery
        chartId="550e8400-e29b-41d4-a716-446655440000"
        isOpen={true}
        onToggle={vi.fn()}
      />,
      { queryClient }
    );

    // Wait for data to load
    await waitFor(() => {
      expect(screen.getByText('Active Collaborators (2)')).toBeInTheDocument();
    });

    // Check that annotations are displayed
    expect(screen.getByText('GDP Growth Analysis')).toBeInTheDocument();
    expect(screen.getByText('Market Trends')).toBeInTheDocument();
  });

  it('should show loading state initially', () => {
    // Mock a slow response
    mockExecuteGraphQL.mockImplementation(() => 
      new Promise(resolve => setTimeout(() => resolve({ data: null }), 100))
    );

    renderWithProviders(
      <ChartCollaborationConnectedQuery
        chartId="550e8400-e29b-41d4-a716-446655440000"
        isOpen={true}
        onToggle={vi.fn()}
      />,
      { queryClient }
    );

    // Should show loading spinner
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
  });

  it('should handle share dialog', async () => {
    renderWithProviders(
      <ChartCollaborationConnectedQuery
        chartId="550e8400-e29b-41d4-a716-446655440000"
        isOpen={true}
        onToggle={vi.fn()}
      />,
      { queryClient }
    );

    await waitFor(() => {
      expect(screen.getByText('Active Collaborators (2)')).toBeInTheDocument();
    });

    // Click share button
    const shareButton = screen.getByTestId('share-chart-button');
    fireEvent.click(shareButton);

    // Should open share dialog
    expect(screen.getByText('Share Chart')).toBeInTheDocument();
    expect(screen.getByLabelText('User Email')).toBeInTheDocument();
  });

  it('should filter annotations correctly', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <ChartCollaborationConnectedQuery
        chartId="550e8400-e29b-41d4-a716-446655440000"
        isOpen={true}
        onToggle={vi.fn()}
      />,
      { queryClient }
    );

    await waitFor(() => {
      expect(screen.getByText('Active Collaborators (2)')).toBeInTheDocument();
    });

    // Wait for annotations to load
    await waitFor(() => {
      expect(screen.getByText('Annotations (2)')).toBeInTheDocument();
    });

    // Check filter dropdown
    const filterSelect = screen.getByTestId('filter-annotations-select');
    expect(filterSelect).toBeInTheDocument();

    // Test filtering by pinned
    await user.click(filterSelect);
    
    // Wait for the dropdown to appear and find the pinned option
    await waitFor(() => {
      expect(screen.getByText('Pinned (1)')).toBeInTheDocument();
    });
    
    const pinnedOption = screen.getByText('Pinned (1)');
    await user.click(pinnedOption);

    // Should only show pinned annotation
    expect(screen.getByText('Market Trends')).toBeInTheDocument();
    expect(screen.queryByText('GDP Growth Analysis')).not.toBeInTheDocument();
  });

  it('should handle annotation selection and comments', async () => {
    renderWithProviders(
      <ChartCollaborationConnectedQuery
        chartId="550e8400-e29b-41d4-a716-446655440000"
        isOpen={true}
        onToggle={vi.fn()}
      />,
      { queryClient }
    );

    await waitFor(() => {
      expect(screen.getByText('Active Collaborators (2)')).toBeInTheDocument();
    });

    // Click on an annotation
    const annotation = screen.getByText('GDP Growth Analysis');
    fireEvent.click(annotation);

    // Should open comments dialog
    await waitFor(() => {
      expect(screen.getByText('Comments for "GDP Growth Analysis"')).toBeInTheDocument();
    });

    // Should show comments
    expect(screen.getByText('Great analysis!')).toBeInTheDocument();
  });

  it('should show error state when user is not authenticated', () => {
    // Mock no user
    vi.doMock('../../../contexts/AuthContext', () => ({
      useAuth: () => ({ user: null }),
    }));

    renderWithProviders(
      <ChartCollaborationConnectedQuery
        chartId="550e8400-e29b-41d4-a716-446655440000"
        isOpen={true}
        onToggle={vi.fn()}
      />,
      { queryClient }
    );

    expect(screen.getByText('Please log in to access collaboration features.')).toBeInTheDocument();
  });

  it('should handle mutations correctly', async () => {
    renderWithProviders(
      <ChartCollaborationConnectedQuery
        chartId="550e8400-e29b-41d4-a716-446655440000"
        isOpen={true}
        onToggle={vi.fn()}
      />,
      { queryClient }
    );

    await waitFor(() => {
      expect(screen.getByText('Active Collaborators (2)')).toBeInTheDocument();
    });

    // Click add annotation button
    const addButton = screen.getByText('Add Annotation');
    fireEvent.click(addButton);

    // Should open annotation dialog
    expect(screen.getByText('Add New Annotation')).toBeInTheDocument();
  });
});
