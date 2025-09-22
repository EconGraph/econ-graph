/**
 * REQUIREMENT: Comprehensive unit tests for ChartCollaboration component
 * PURPOSE: Test chart collaboration features including annotations, comments, and filtering
 * This ensures the collaboration functionality works correctly for professional economic analysis
 */

import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import ChartCollaboration, { ChartAnnotation, ChartComment } from '../ChartCollaboration';

// Mock date-fns format function
jest.mock('date-fns', () => ({
  format: jest.fn((date) => 'Jan 15, 2:30 PM'),
}));

const theme = createTheme();

// Mock data for testing
const mockCurrentUser = {
  id: 'user-1',
  name: 'Test User',
  avatar: 'https://example.com/avatar.jpg',
};

const mockCollaborators = [
  {
    id: 'collab-1',
    name: 'John Doe',
    avatar: 'https://example.com/john.jpg',
    isOnline: true,
    role: 'editor' as const,
  },
  {
    id: 'collab-2',
    name: 'Jane Smith',
    avatar: 'https://example.com/jane.jpg',
    isOnline: false,
    role: 'viewer' as const,
  },
];

const mockAnnotations: ChartAnnotation[] = [
  {
    id: 'annotation-1',
    date: '2024-01-15',
    value: 100.5,
    title: 'GDP Growth Spike',
    description: 'Significant increase in GDP growth rate',
    color: '#f44336',
    type: 'line',
    author: {
      id: 'user-1',
      name: 'Test User',
      avatar: 'https://example.com/avatar.jpg',
    },
    createdAt: '2024-01-15T14:30:00Z',
    updatedAt: '2024-01-15T14:30:00Z',
    isVisible: true,
    isPinned: false,
    tags: ['gdp', 'growth'],
    comments: [],
  },
  {
    id: 'annotation-2',
    date: '2024-01-16',
    value: 98.2,
    title: 'Market Correction',
    description: 'Expected market correction period',
    color: '#2196f3',
    type: 'point',
    author: {
      id: 'collab-1',
      name: 'John Doe',
      avatar: 'https://example.com/john.jpg',
    },
    createdAt: '2024-01-16T10:15:00Z',
    updatedAt: '2024-01-16T10:15:00Z',
    isVisible: true,
    isPinned: true,
    tags: ['market', 'correction'],
    comments: [
      {
        id: 'comment-1',
        content: 'This looks like a temporary dip',
        author: {
          id: 'collab-2',
          name: 'Jane Smith',
          avatar: 'https://example.com/jane.jpg',
        },
        createdAt: '2024-01-16T11:00:00Z',
        isResolved: false,
      },
    ],
  },
];

// Mock handlers
const mockHandlers = {
  onAnnotationAdd: jest.fn(),
  onAnnotationUpdate: jest.fn(),
  onAnnotationDelete: jest.fn(),
  onCommentAdd: jest.fn(),
  onToggle: jest.fn(),
};

const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <ThemeProvider theme={theme}>
    {children}
  </ThemeProvider>
);

describe('ChartCollaboration', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  const renderChartCollaboration = (props = {}) => {
    const defaultProps = {
      annotations: mockAnnotations,
      currentUser: mockCurrentUser,
      collaborators: mockCollaborators,
      isOpen: true,
      ...mockHandlers,
      ...props,
    };

    return render(
      <TestWrapper>
        <ChartCollaboration {...defaultProps} />
      </TestWrapper>
    );
  };

  describe('Basic Rendering', () => {
    it('should render collaboration panel when open', () => {
      renderChartCollaboration();

      expect(screen.getByText('Chart Collaboration')).toBeInTheDocument();
      expect(screen.getByText('Add Annotation')).toBeInTheDocument();
      expect(screen.getByText('Filter Annotations')).toBeInTheDocument();
    });

    it('should not render when closed', () => {
      renderChartCollaboration({ isOpen: false });

      expect(screen.queryByText('Chart Collaboration')).not.toBeInTheDocument();
    });

    it('should display active collaborators', () => {
      renderChartCollaboration();

      expect(screen.getByText('Active Collaborators (1)')).toBeInTheDocument();
      expect(screen.getByText('John Doe')).toBeInTheDocument();
    });

    it('should show total comment count', () => {
      renderChartCollaboration();

      // Should show badge with total comments (1 from annotation-2)
      const commentBadge = screen.getByText('1');
      expect(commentBadge).toBeInTheDocument();
    });
  });

  describe('Annotation Display', () => {
    it('should display all annotations by default', () => {
      renderChartCollaboration();

      expect(screen.getByText('GDP Growth Spike')).toBeInTheDocument();
      expect(screen.getByText('Market Correction')).toBeInTheDocument();
      expect(screen.getByText('Annotations (2)')).toBeInTheDocument();
    });

    it('should show annotation details correctly', () => {
      renderChartCollaboration();

      // Check first annotation
      expect(screen.getByText('GDP Growth Spike')).toBeInTheDocument();
      expect(screen.getByText('Significant increase in GDP growth rate')).toBeInTheDocument();
      expect(screen.getByText('Test User • Jan 15, 2:30 PM')).toBeInTheDocument();

      // Check second annotation
      expect(screen.getByText('Market Correction')).toBeInTheDocument();
      expect(screen.getByText('Expected market correction period')).toBeInTheDocument();
      expect(screen.getByText('John Doe • Jan 15, 2:30 PM')).toBeInTheDocument();
    });

    it('should display annotation tags', () => {
      renderChartCollaboration();

      expect(screen.getByText('gdp')).toBeInTheDocument();
      expect(screen.getByText('growth')).toBeInTheDocument();
      expect(screen.getByText('market')).toBeInTheDocument();
      expect(screen.getByText('correction')).toBeInTheDocument();
    });

    it('should show comment count for annotations with comments', () => {
      renderChartCollaboration();

      expect(screen.getByText('1 comment')).toBeInTheDocument();
    });

    it('should show pinned annotation indicator', () => {
      renderChartCollaboration();

      // Should show pin icon for pinned annotation
      const pinIcons = screen.getAllByTestId('PushPinIcon');
      expect(pinIcons.length).toBeGreaterThan(0);
    });
  });

  describe('Annotation Filtering', () => {
    it('should filter to show only user annotations', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      const filterSelect = screen.getByLabelText('Filter Annotations');
      await user.click(filterSelect);
      await user.click(screen.getByText('My Annotations (1)'));

      // Should only show user's annotations
      expect(screen.getByText('GDP Growth Spike')).toBeInTheDocument();
      expect(screen.queryByText('Market Correction')).not.toBeInTheDocument();
      expect(screen.getByText('Annotations (1)')).toBeInTheDocument();
    });

    it('should filter to show only pinned annotations', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      const filterSelect = screen.getByLabelText('Filter Annotations');
      await user.click(filterSelect);
      await user.click(screen.getByText('Pinned (1)'));

      // Should only show pinned annotations
      expect(screen.queryByText('GDP Growth Spike')).not.toBeInTheDocument();
      expect(screen.getByText('Market Correction')).toBeInTheDocument();
      expect(screen.getByText('Annotations (1)')).toBeInTheDocument();
    });

    it('should show all annotations when filter is reset', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // First filter to pinned
      const filterSelect = screen.getByLabelText('Filter Annotations');
      await user.click(filterSelect);
      await user.click(screen.getByText('Pinned (1)'));

      // Then reset to all
      await user.click(filterSelect);
      await user.click(screen.getByText('All Annotations (2)'));

      // Should show all annotations again
      expect(screen.getByText('GDP Growth Spike')).toBeInTheDocument();
      expect(screen.getByText('Market Correction')).toBeInTheDocument();
      expect(screen.getByText('Annotations (2)')).toBeInTheDocument();
    });
  });

  describe('Annotation Creation', () => {
    it('should open annotation creation dialog', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);

      expect(screen.getByText('Add Chart Annotation')).toBeInTheDocument();
      expect(screen.getByLabelText('Title')).toBeInTheDocument();
      expect(screen.getByLabelText('Description')).toBeInTheDocument();
      expect(screen.getByLabelText('Date (YYYY-MM-DD)')).toBeInTheDocument();
    });

    it('should create annotation with valid data', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open dialog
      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);

      // Fill form
      await user.type(screen.getByLabelText('Title'), 'New Annotation');
      await user.type(screen.getByLabelText('Description'), 'This is a test annotation');
      await user.type(screen.getByLabelText('Date (YYYY-MM-DD)'), '2024-01-20');
      await user.type(screen.getByLabelText('Value (optional)'), '105.5');

      // Select annotation type
      const typeSelect = screen.getByLabelText('Annotation Type');
      await user.click(typeSelect);
      await user.click(screen.getByText('● Data Point'));

      // Add tags
      await user.type(screen.getByLabelText('Tags (comma-separated)'), 'test, analysis');

      // Submit
      const submitButton = screen.getByText('Add Annotation');
      await user.click(submitButton);

      expect(mockHandlers.onAnnotationAdd).toHaveBeenCalledWith({
        date: '2024-01-20',
        value: 105.5,
        title: 'New Annotation',
        description: 'This is a test annotation',
        color: '#f44336', // Default color
        type: 'point',
        author: mockCurrentUser,
        isVisible: true,
        isPinned: false,
        tags: ['test', 'analysis'],
        comments: [],
      });
    });

    it('should not create annotation without required fields', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open dialog
      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);

      // Try to submit without title
      const submitButton = screen.getByText('Add Annotation');
      await user.click(submitButton);

      // Should not call onAnnotationAdd
      expect(mockHandlers.onAnnotationAdd).not.toHaveBeenCalled();
      // Dialog should still be open
      expect(screen.getByText('Add Chart Annotation')).toBeInTheDocument();
    });

    it('should reset form when dialog is closed', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open dialog and fill some data
      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);
      await user.type(screen.getByLabelText('Title'), 'Test Title');

      // Close dialog
      const cancelButton = screen.getByText('Cancel');
      await user.click(cancelButton);

      // Reopen dialog
      await user.click(addButton);

      // Form should be reset
      expect(screen.getByLabelText('Title')).toHaveValue('');
    });
  });

  describe('Annotation Interactions', () => {
    it('should toggle annotation visibility', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Find visibility toggle button for first annotation
      const visibilityButtons = screen.getAllByTestId('VisibilityIcon');
      await user.click(visibilityButtons[0]);

      expect(mockHandlers.onAnnotationUpdate).toHaveBeenCalledWith(
        'annotation-1',
        { isVisible: false }
      );
    });

    it('should toggle annotation pin status', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Find pin button for first annotation
      const pinButtons = screen.getAllByTestId('PushPinIcon');
      await user.click(pinButtons[0]);

      expect(mockHandlers.onAnnotationUpdate).toHaveBeenCalledWith(
        'annotation-1',
        { isPinned: true }
      );
    });

    it('should open comments dialog when comment button is clicked', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Find comment button for first annotation
      const commentButtons = screen.getAllByTestId('CommentIcon');
      await user.click(commentButtons[0]);

      expect(screen.getByText('GDP Growth Spike - Comments')).toBeInTheDocument();
      expect(screen.getByText('Significant increase in GDP growth rate')).toBeInTheDocument();
    });

    it('should delete annotation when delete button is clicked', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Find delete button for first annotation (user's own annotation)
      const deleteButtons = screen.getAllByTestId('DeleteIcon');
      await user.click(deleteButtons[0]);

      expect(mockHandlers.onAnnotationDelete).toHaveBeenCalledWith('annotation-1');
    });

    it('should not show delete button for other users annotations', () => {
      renderChartCollaboration();

      // Should only show delete button for user's own annotations
      const deleteButtons = screen.getAllByTestId('DeleteIcon');
      expect(deleteButtons).toHaveLength(1); // Only for user's own annotation
    });
  });

  describe('Comments Functionality', () => {
    it('should display existing comments', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open comments for annotation with comments
      const commentButtons = screen.getAllByTestId('CommentIcon');
      await user.click(commentButtons[1]); // Second annotation has comments

      expect(screen.getByText('This looks like a temporary dip')).toBeInTheDocument();
      expect(screen.getByText('Jane Smith')).toBeInTheDocument();
    });

    it('should add new comment', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open comments dialog
      const commentButtons = screen.getAllByTestId('CommentIcon');
      await user.click(commentButtons[0]);

      // Add comment
      const commentInput = screen.getByLabelText('Add a comment...');
      await user.type(commentInput, 'This is a new comment');

      const commentButton = screen.getByText('Comment');
      await user.click(commentButton);

      expect(mockHandlers.onCommentAdd).toHaveBeenCalledWith('annotation-1', {
        content: 'This is a new comment',
        author: mockCurrentUser,
        isResolved: false,
      });
    });

    it('should not add empty comment', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open comments dialog
      const commentButtons = screen.getAllByTestId('CommentIcon');
      await user.click(commentButtons[0]);

      // Try to add empty comment
      const commentButton = screen.getByText('Comment');
      expect(commentButton).toBeDisabled();
    });

    it('should clear comment input after adding', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open comments dialog
      const commentButtons = screen.getAllByTestId('CommentIcon');
      await user.click(commentButtons[0]);

      // Add comment
      const commentInput = screen.getByLabelText('Add a comment...');
      await user.type(commentInput, 'Test comment');

      const commentButton = screen.getByText('Comment');
      await user.click(commentButton);

      // Input should be cleared
      expect(commentInput).toHaveValue('');
    });
  });

  describe('Color Selection', () => {
    it('should allow color selection in annotation form', async () => {
      const user = userEvent.setup();
      renderChartCollaboration();

      // Open annotation dialog
      const addButton = screen.getByText('Add Annotation');
      await user.click(addButton);

      // Find color selection area
      expect(screen.getByText('Color')).toBeInTheDocument();

      // Should have color options
      const colorBoxes = screen.getAllByRole('button', { hidden: true });
      const colorBox = colorBoxes.find(box =>
        box.style.backgroundColor === '#e91e63' ||
        box.getAttribute('style')?.includes('#e91e63')
      );

      if (colorBox) {
        await user.click(colorBox);
      }
    });
  });

  describe('Empty States', () => {
    it('should show empty state when no annotations', () => {
      renderChartCollaboration({ annotations: [] });

      expect(screen.getByText('No annotations found. Click "Add Annotation" to create your first annotation.')).toBeInTheDocument();
      expect(screen.getByText('Annotations (0)')).toBeInTheDocument();
    });

    it('should show empty state when filtered annotations are empty', async () => {
      const user = userEvent.setup();

      // Create annotations where user has none
      const otherUserAnnotations = mockAnnotations.map(ann => ({
        ...ann,
        author: { ...ann.author, id: 'other-user' }
      }));

      renderChartCollaboration({ annotations: otherUserAnnotations });

      // Filter to user's annotations
      const filterSelect = screen.getByLabelText('Filter Annotations');
      await user.click(filterSelect);
      await user.click(screen.getByText('My Annotations (0)'));

      expect(screen.getByText('No annotations found. Click "Add Annotation" to create your first annotation.')).toBeInTheDocument();
    });
  });

  describe('Collaborator Display', () => {
    it('should show online status for collaborators', () => {
      renderChartCollaboration();

      // Should show online indicator for active collaborators
      const onlineBadges = screen.getAllByTestId('Badge');
      expect(onlineBadges.length).toBeGreaterThan(0);
    });

    it('should show collaborator count correctly', () => {
      renderChartCollaboration();

      expect(screen.getByText('Active Collaborators (1)')).toBeInTheDocument();
    });

    it('should show overflow indicator when many collaborators', () => {
      const manyCollaborators = Array.from({ length: 10 }, (_, i) => ({
        id: `collab-${i}`,
        name: `User ${i}`,
        avatar: `https://example.com/user${i}.jpg`,
        isOnline: i < 5,
        role: 'viewer' as const,
      }));

      renderChartCollaboration({ collaborators: manyCollaborators });

      // Should show +5 for overflow
      expect(screen.getByText('+5')).toBeInTheDocument();
    });
  });
});
