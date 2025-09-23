/**
 * REQUIREMENT: Integration tests for ChartCollaborationConnected event handlers
 * PURPOSE: Test event handlers with mocked dependencies
 * This ensures business logic works correctly with mocked hooks and APIs
 */

import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { CssBaseline, StyledEngineProvider } from '@mui/material';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import ChartCollaborationConnected from '../ChartCollaborationConnected';
import { ChartAnnotationType } from '../../../utils/graphql';
import { useCollaboration } from '../../../hooks/useCollaboration';

// Mock the useCollaboration hook
jest.mock('../../../hooks/useCollaboration', () => ({
  useCollaboration: jest.fn(),
}));

// Mock the useAuth hook
const mockUseAuth = jest.fn();
jest.mock('../../../contexts/AuthContext', () => ({
  useAuth: () => mockUseAuth(),
}));

// Mock date-fns format function
jest.mock('date-fns', () => ({
  format: jest.fn((date) => 'Jan 15, 2:30 PM'),
}));

const theme = createTheme();

// Mock data
const mockUser = {
  id: 'user-1',
  name: 'Test User',
  email: 'test@example.com',
  avatarUrl: 'https://example.com/avatar.jpg',
};

const mockAnnotations: ChartAnnotationType[] = [
  {
    id: 'annotation-1',
    seriesId: 'series-1',
    userId: 'user-1',
    title: 'Test Annotation',
    description: 'Test description',
    annotationDate: '2024-01-15',
    annotationValue: 100.5,
    annotationType: 'note',
    color: '#f44336',
    isPinned: false,
    isVisible: true,
    createdAt: '2024-01-15T10:00:00Z',
    updatedAt: '2024-01-15T10:00:00Z',
  },
];

const mockCollaborators = [
  {
    id: 'collab-1',
    userId: 'user-1',
    chartId: 'chart-1',
    role: 'admin',
    lastAccessedAt: new Date().toISOString(),
    createdAt: '2024-01-15T10:00:00Z',
    updatedAt: '2024-01-15T10:00:00Z',
  },
];

const mockUsers = {
  'user-1': {
    id: 'user-1',
    name: 'Test User',
    email: 'test@example.com',
    avatarUrl: 'https://example.com/avatar.jpg',
  },
};

// Mock collaboration hook functions
const mockCreateAnnotation = jest.fn();
const mockAddComment = jest.fn();
const mockShareChart = jest.fn();
const mockDeleteAnnotation = jest.fn();
const mockToggleAnnotationVisibility = jest.fn();
const mockToggleAnnotationPin = jest.fn();
const mockLoadComments = jest.fn();
const mockGetUserById = jest.fn();
const mockGetCommentsForAnnotation = jest.fn();

const mockCollaborationHook = {
  annotations: mockAnnotations,
  collaborators: mockCollaborators,
  users: mockUsers,
  loading: false,
  error: null,
  createAnnotation: mockCreateAnnotation,
  addComment: mockAddComment,
  shareChart: mockShareChart,
  deleteAnnotation: mockDeleteAnnotation,
  toggleAnnotationVisibility: mockToggleAnnotationVisibility,
  toggleAnnotationPin: mockToggleAnnotationPin,
  loadComments: mockLoadComments,
  getUserById: mockGetUserById,
  getCommentsForAnnotation: mockGetCommentsForAnnotation,
};

const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <StyledEngineProvider injectFirst>
    <ThemeProvider theme={theme}>
      <LocalizationProvider dateAdapter={AdapterDateFns}>
        <CssBaseline />
        {children}
      </LocalizationProvider>
    </ThemeProvider>
  </StyledEngineProvider>
);

describe('ChartCollaborationConnected - Integration Tests', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockUseAuth.mockReturnValue({ user: mockUser });
    (useCollaboration as jest.Mock).mockReturnValue(mockCollaborationHook);

    // Mock window.confirm
    window.confirm = jest.fn(() => true);
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  const renderChartCollaborationConnected = (props = {}) => {
    const defaultProps = {
      seriesId: 'series-1',
      chartId: 'chart-1',
      isOpen: true,
      onToggle: jest.fn(),
      onAnnotationClick: jest.fn(),
      ...props,
    };

    return render(
      <TestWrapper>
        <ChartCollaborationConnected {...defaultProps} />
      </TestWrapper>
    );
  };

  describe('handleCreateAnnotation', () => {
    it('should validate required fields and show error', async () => {
      const user = userEvent.setup();
      renderChartCollaborationConnected();

      // Open dialog
      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);

      // Try to submit without filling required fields
      const submitButton = screen.getByTestId('submit-annotation-button');
      await user.click(submitButton);

      // Should show error snackbar
      await waitFor(() => {
        expect(screen.getByText('Title and content are required')).toBeInTheDocument();
      });

      // Should not call createAnnotation
      expect(mockCreateAnnotation).not.toHaveBeenCalled();
    });

    it('should call createAnnotation with correct data when form is valid', async () => {
      const user = userEvent.setup();
      mockCreateAnnotation.mockResolvedValue(undefined);
      renderChartCollaborationConnected();

      // Open dialog
      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);

      // Fill form
      await user.type(screen.getByRole('textbox', { name: /title/i }), 'Test Title');
      await user.type(screen.getByRole('textbox', { name: /content/i }), 'Test Content');

      // Submit
      const submitButton = screen.getByTestId('submit-annotation-button');
      await user.click(submitButton);

      // Should call createAnnotation with correct data
      await waitFor(() => {
        expect(mockCreateAnnotation).toHaveBeenCalledWith({
          series_id: 'series-1',
          annotation_date: expect.any(String),
          annotation_value: undefined,
          title: 'Test Title',
          content: 'Test Content',
          annotation_type: 'note',
          color: '#f44336',
          is_public: true,
        });
      });
    });

    it('should handle createAnnotation error', async () => {
      const user = userEvent.setup();
      const errorMessage = 'Failed to create annotation';
      mockCreateAnnotation.mockRejectedValue(new Error(errorMessage));
      renderChartCollaborationConnected();

      // Open dialog
      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);

      // Fill form
      await user.type(screen.getByRole('textbox', { name: /title/i }), 'Test Title');
      await user.type(screen.getByRole('textbox', { name: /content/i }), 'Test Content');

      // Submit
      const submitButton = screen.getByTestId('submit-annotation-button');
      await user.click(submitButton);

      // Should show error snackbar
      await waitFor(() => {
        expect(screen.getByText(errorMessage)).toBeInTheDocument();
      });
    });
  });

  describe('handleAddComment', () => {
    it('should call addComment with correct data', async () => {
      const user = userEvent.setup();
      mockAddComment.mockResolvedValue(undefined);
      renderChartCollaborationConnected();

      // Select an annotation first
      const annotationItem = screen.getByText('Test Annotation');
      await user.click(annotationItem);

      // Wait for comments dialog to open
      await waitFor(() => {
        expect(screen.getByText('Test Annotation - Comments')).toBeInTheDocument();
      });

      // Add comment
      const commentInput = screen.getByTestId('comment-input');
      await user.type(commentInput, 'Test comment');

      const addButton = screen.getByText('Comment');
      await user.click(addButton);

      // Should call addComment
      await waitFor(() => {
        expect(mockAddComment).toHaveBeenCalledWith('annotation-1', 'Test comment');
      });
    });

    it('should handle addComment error', async () => {
      const user = userEvent.setup();
      const errorMessage = 'Failed to add comment';
      mockAddComment.mockRejectedValue(new Error(errorMessage));
      renderChartCollaborationConnected();

      // Select an annotation first
      const annotationItem = screen.getByText('Test Annotation');
      await user.click(annotationItem);

      // Wait for comments dialog to open
      await waitFor(() => {
        expect(screen.getByText('Test Annotation - Comments')).toBeInTheDocument();
      });

      // Add comment
      const commentInput = screen.getByTestId('comment-input');
      await user.type(commentInput, 'Test comment');

      const addButton = screen.getByText('Comment');
      await user.click(addButton);

      // Should show error snackbar
      await waitFor(() => {
        expect(screen.getByText(errorMessage)).toBeInTheDocument();
      });
    });
  });

  describe('handleShareChart', () => {
    it('should validate required fields and show error', async () => {
      const user = userEvent.setup();
      renderChartCollaborationConnected();

      // Open share dialog
      const shareButton = screen.getByRole('button', { name: /share/i });
      await user.click(shareButton);

      // Try to submit without filling required fields
      const submitButton = screen.getByTestId('submit-share-button');
      await user.click(submitButton);

      // Should show error snackbar
      await waitFor(() => {
        expect(screen.getByText('Chart ID and target user are required')).toBeInTheDocument();
      });

      // Should not call shareChart
      expect(mockShareChart).not.toHaveBeenCalled();
    });

    it('should call shareChart with correct data when form is valid', async () => {
      const user = userEvent.setup();
      mockShareChart.mockResolvedValue(undefined);
      renderChartCollaborationConnected();

      // Open share dialog
      const shareButton = screen.getByRole('button', { name: /share/i });
      await user.click(shareButton);

      // Fill form - just type in the user input field directly
      const userInput = screen.getByTestId('share-target-user-input');
      await user.type(userInput, 'user-1');

      // Submit
      const submitButton = screen.getByTestId('submit-share-button');
      await user.click(submitButton);

      // Should call shareChart
      await waitFor(() => {
        expect(mockShareChart).toHaveBeenCalledWith({
          target_user_id: 'user-1',
          chart_id: 'chart-1',
          permission_level: 'view',
        });
      });
    });
  });

  describe('handleDeleteAnnotation', () => {
    it('should call deleteAnnotation when confirmed', async () => {
      const user = userEvent.setup();
      mockDeleteAnnotation.mockResolvedValue(undefined);
      renderChartCollaborationConnected();

      // Find and click delete button for annotation
      const deleteButton = screen.getByRole('button', { name: /delete/i });
      await user.click(deleteButton);

      // Should call deleteAnnotation
      await waitFor(() => {
        expect(mockDeleteAnnotation).toHaveBeenCalledWith('annotation-1');
      });
    });

    it('should not call deleteAnnotation when cancelled', async () => {
      const user = userEvent.setup();
      window.confirm = jest.fn(() => false); // User cancels
      renderChartCollaborationConnected();

      // Find and click delete button for annotation
      const deleteButton = screen.getByRole('button', { name: /delete/i });
      await user.click(deleteButton);

      // Should not call deleteAnnotation
      expect(mockDeleteAnnotation).not.toHaveBeenCalled();
    });

    it('should handle deleteAnnotation error', async () => {
      const user = userEvent.setup();
      const errorMessage = 'Failed to delete annotation';
      mockDeleteAnnotation.mockRejectedValue(new Error(errorMessage));
      renderChartCollaborationConnected();

      // Find and click delete button for annotation
      const deleteButton = screen.getByRole('button', { name: /delete/i });
      await user.click(deleteButton);

      // Should show error snackbar
      await waitFor(() => {
        expect(screen.getByText(errorMessage)).toBeInTheDocument();
      });
    });
  });

  describe('handleAnnotationSelect', () => {
    it('should call loadComments and onAnnotationClick when annotation is selected', async () => {
      const user = userEvent.setup();
      const mockOnAnnotationClick = jest.fn();
      mockLoadComments.mockResolvedValue(undefined);
      renderChartCollaborationConnected({ onAnnotationClick: mockOnAnnotationClick });

      // Click on annotation
      const annotationItem = screen.getByText('Test Annotation');
      await user.click(annotationItem);

      // Should call loadComments
      await waitFor(() => {
        expect(mockLoadComments).toHaveBeenCalledWith('annotation-1');
      });

      // Should call onAnnotationClick
      expect(mockOnAnnotationClick).toHaveBeenCalledWith(mockAnnotations[0]);
    });
  });

  describe('Loading and Error States', () => {
    it('should handle loading state', () => {
      (useCollaboration as jest.Mock).mockReturnValue({
        ...mockCollaborationHook,
        loading: true,
      });

      renderChartCollaborationConnected();

      // Should show loading indicators
      expect(screen.getAllByLabelText('Loading...')).toHaveLength(1); // Collaborators
    });

    it('should handle error state', () => {
      (useCollaboration as jest.Mock).mockReturnValue({
        ...mockCollaborationHook,
        error: 'Failed to load data',
      });

      renderChartCollaborationConnected();

      // Should show error message
      expect(screen.getByText('Failed to load data')).toBeInTheDocument();
    });
  });
});
