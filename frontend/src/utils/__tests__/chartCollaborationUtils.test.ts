/**
 * REQUIREMENT: Unit tests for chart collaboration business logic
 * PURPOSE: Test pure functions extracted from ChartCollaborationConnected component
 * This ensures business logic works correctly without UI dependencies
 */

import {
  filterAnnotations,
  calculateTotalComments,
  filterActiveCollaborators,
  validateAnnotationForm,
  validateShareForm,
  formatAnnotationForApi,
  getDefaultAnnotationForm,
  getDefaultShareForm,
  // AnnotationFilter,
  Collaborator,
  User,
} from '../chartCollaborationUtils';
import { ChartAnnotationType } from '../../utils/graphql';

describe('chartCollaborationUtils', () => {
  // Mock data
  const mockAnnotations: ChartAnnotationType[] = [
    {
      id: 'annotation-1',
      seriesId: 'series-1',
      userId: 'user-1',
      title: 'Test Annotation 1',
      description: 'Description 1',
      annotationDate: '2024-01-15',
      annotationValue: 100.5,
      annotationType: 'note',
      color: '#f44336',
      isPinned: false,
      isVisible: true,
      createdAt: '2024-01-15T10:00:00Z',
      updatedAt: '2024-01-15T10:00:00Z',
    },
    {
      id: 'annotation-2',
      seriesId: 'series-1',
      userId: 'user-2',
      title: 'Test Annotation 2',
      description: 'Description 2',
      annotationDate: '2024-01-16',
      annotationValue: 200.5,
      annotationType: 'analysis',
      color: '#2196f3',
      isPinned: true,
      isVisible: true,
      createdAt: '2024-01-16T10:00:00Z',
      updatedAt: '2024-01-16T10:00:00Z',
    },
    {
      id: 'annotation-3',
      seriesId: 'series-1',
      userId: 'user-1',
      title: 'Test Annotation 3',
      description: 'Description 3',
      annotationDate: '2024-01-17',
      annotationValue: 300.5,
      annotationType: 'warning',
      color: '#ff9800',
      isPinned: false,
      isVisible: false,
      createdAt: '2024-01-17T10:00:00Z',
      updatedAt: '2024-01-17T10:00:00Z',
    },
  ];

  const mockCollaborators: Collaborator[] = [
    {
      id: 'collab-1',
      userId: 'user-1',
      chartId: 'chart-1',
      role: 'admin',
      lastAccessedAt: new Date(Date.now() - 60000).toISOString(), // 1 minute ago
      createdAt: '2024-01-15T10:00:00Z',
      updatedAt: '2024-01-15T10:00:00Z',
    },
    {
      id: 'collab-2',
      userId: 'user-2',
      chartId: 'chart-1',
      role: 'editor',
      lastAccessedAt: new Date(Date.now() - 300000).toISOString(), // 5 minutes ago
      createdAt: '2024-01-15T11:00:00Z',
      updatedAt: '2024-01-15T11:00:00Z',
    },
  ];

  const mockUsers: Record<string, User> = {
    'user-1': {
      id: 'user-1',
      name: 'User One',
      email: 'user1@example.com',
      avatarUrl: 'https://example.com/user1.jpg',
    },
    'user-2': {
      id: 'user-2',
      name: 'User Two',
      email: 'user2@example.com',
      avatarUrl: 'https://example.com/user2.jpg',
    },
  };

  describe('filterAnnotations', () => {
    it('should filter annotations by "all" (default)', () => {
      const result = filterAnnotations(mockAnnotations, 'all');
      expect(result).toHaveLength(2); // Only visible annotations
      expect(result.map(a => a.id)).toEqual(['annotation-1', 'annotation-2']);
    });

    it('should filter annotations by "mine"', () => {
      const result = filterAnnotations(mockAnnotations, 'mine', 'user-1');
      expect(result).toHaveLength(2); // Both annotation-1 and annotation-3 belong to user-1
      expect(result.map(a => a.id)).toEqual(['annotation-1', 'annotation-3']);
    });

    it('should filter annotations by "pinned"', () => {
      const result = filterAnnotations(mockAnnotations, 'pinned');
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('annotation-2');
    });

    it('should return empty array when no annotations match filter', () => {
      const result = filterAnnotations(mockAnnotations, 'mine', 'user-3');
      expect(result).toHaveLength(0);
    });

    it('should handle empty annotations array', () => {
      const result = filterAnnotations([], 'all');
      expect(result).toHaveLength(0);
    });
  });

  describe('calculateTotalComments', () => {
    it('should calculate total comments correctly', () => {
      const mockGetCommentsForAnnotation = jest.fn()
        .mockReturnValueOnce([{ id: 'comment-1' }, { id: 'comment-2' }]) // annotation-1: 2 comments
        .mockReturnValueOnce([{ id: 'comment-3' }]) // annotation-2: 1 comment
        .mockReturnValueOnce([]); // annotation-3: 0 comments

      const result = calculateTotalComments(mockAnnotations, mockGetCommentsForAnnotation);
      expect(result).toBe(3);
      expect(mockGetCommentsForAnnotation).toHaveBeenCalledTimes(3);
    });

    it('should handle undefined comments gracefully', () => {
      const mockGetCommentsForAnnotation = jest.fn()
        .mockReturnValueOnce(undefined)
        .mockReturnValueOnce(null);

      const result = calculateTotalComments(mockAnnotations.slice(0, 2), mockGetCommentsForAnnotation);
      expect(result).toBe(0);
    });

    it('should handle empty annotations array', () => {
      const mockGetCommentsForAnnotation = jest.fn();
      const result = calculateTotalComments([], mockGetCommentsForAnnotation);
      expect(result).toBe(0);
      expect(mockGetCommentsForAnnotation).not.toHaveBeenCalled();
    });
  });

  describe('filterActiveCollaborators', () => {
    it('should filter active collaborators by default threshold (5 minutes)', () => {
      const result = filterActiveCollaborators(mockCollaborators, mockUsers);
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('collab-1'); // Only the one from 1 minute ago
    });

    it('should filter active collaborators by custom threshold', () => {
      const result = filterActiveCollaborators(mockCollaborators, mockUsers, 600000); // 10 minutes
      expect(result).toHaveLength(2);
      expect(result.map(c => c.id)).toEqual(['collab-1', 'collab-2']);
    });

    it('should exclude collaborators without user data', () => {
      const collaboratorsWithUnknownUser = [
        ...mockCollaborators,
        {
          id: 'collab-3',
          userId: 'user-3', // No user data
          chartId: 'chart-1',
          role: 'viewer',
          lastAccessedAt: new Date(Date.now() - 60000).toISOString(),
          createdAt: '2024-01-15T12:00:00Z',
          updatedAt: '2024-01-15T12:00:00Z',
        },
      ];

      const result = filterActiveCollaborators(collaboratorsWithUnknownUser, mockUsers);
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('collab-1');
    });

    it('should handle empty collaborators array', () => {
      const result = filterActiveCollaborators([], mockUsers);
      expect(result).toHaveLength(0);
    });
  });

  describe('validateAnnotationForm', () => {
    it('should validate correct form data', () => {
      const formData = {
        title: 'Test Title',
        content: 'Test Content',
        annotationDate: '2024-01-15',
        annotationValue: '100.5',
        annotationType: 'note',
      };

      const result = validateAnnotationForm(formData);
      expect(result.isValid).toBe(true);
      expect(result.errors).toHaveLength(0);
    });

    it('should detect missing title', () => {
      const formData = {
        title: '',
        content: 'Test Content',
        annotationDate: '2024-01-15',
        annotationType: 'note',
      };

      const result = validateAnnotationForm(formData);
      expect(result.isValid).toBe(false);
      expect(result.errors).toContain('Title is required');
    });

    it('should detect missing content', () => {
      const formData = {
        title: 'Test Title',
        content: '',
        annotationDate: '2024-01-15',
        annotationType: 'note',
      };

      const result = validateAnnotationForm(formData);
      expect(result.isValid).toBe(false);
      expect(result.errors).toContain('Content is required');
    });

    it('should detect invalid annotation value', () => {
      const formData = {
        title: 'Test Title',
        content: 'Test Content',
        annotationDate: '2024-01-15',
        annotationValue: 'not-a-number',
        annotationType: 'note',
      };

      const result = validateAnnotationForm(formData);
      expect(result.isValid).toBe(false);
      expect(result.errors).toContain('Value must be a valid number');
    });

    it('should detect multiple validation errors', () => {
      const formData = {
        title: '',
        content: '',
        annotationDate: '',
        annotationType: '',
      };

      const result = validateAnnotationForm(formData);
      expect(result.isValid).toBe(false);
      expect(result.errors).toHaveLength(4);
    });
  });

  describe('validateShareForm', () => {
    it('should validate correct share form data', () => {
      const formData = {
        targetUserId: 'user-2',
        permissionLevel: 'view',
      };

      const result = validateShareForm(formData, 'chart-1');
      expect(result.isValid).toBe(true);
      expect(result.errors).toHaveLength(0);
    });

    it('should detect missing chart ID', () => {
      const formData = {
        targetUserId: 'user-2',
        permissionLevel: 'view',
      };

      const result = validateShareForm(formData);
      expect(result.isValid).toBe(false);
      expect(result.errors).toContain('Chart ID is required');
    });

    it('should detect missing target user', () => {
      const formData = {
        targetUserId: '',
        permissionLevel: 'view',
      };

      const result = validateShareForm(formData, 'chart-1');
      expect(result.isValid).toBe(false);
      expect(result.errors).toContain('Target user is required');
    });

    it('should detect missing permission level', () => {
      const formData = {
        targetUserId: 'user-2',
        permissionLevel: '',
      };

      const result = validateShareForm(formData, 'chart-1');
      expect(result.isValid).toBe(false);
      expect(result.errors).toContain('Permission level is required');
    });
  });

  describe('formatAnnotationForApi', () => {
    it('should format annotation data correctly', () => {
      const formData = {
        title: 'Test Title',
        content: 'Test Content',
        annotationDate: '2024-01-15',
        annotationValue: '100.5',
        annotationType: 'note',
        color: '#f44336',
        isPublic: true,
      };

      const result = formatAnnotationForApi(formData, 'series-1');
      expect(result).toEqual({
        series_id: 'series-1',
        annotation_date: '2024-01-15',
        annotation_value: 100.5,
        title: 'Test Title',
        content: 'Test Content',
        annotation_type: 'note',
        color: '#f44336',
        is_public: true,
      });
    });

    it('should handle undefined annotation value', () => {
      const formData = {
        title: 'Test Title',
        content: 'Test Content',
        annotationDate: '2024-01-15',
        annotationValue: '',
        annotationType: 'note',
        color: '#f44336',
        isPublic: true,
      };

      const result = formatAnnotationForApi(formData, 'series-1');
      expect(result.annotation_value).toBeUndefined();
    });
  });

  describe('getDefaultAnnotationForm', () => {
    it('should return default annotation form data', () => {
      const result = getDefaultAnnotationForm();
      expect(result).toEqual({
        annotationDate: expect.any(String),
        annotationValue: '',
        title: '',
        content: '',
        color: '#f44336',
        annotationType: 'note',
        isPublic: true,
      });
      expect(result.annotationDate).toMatch(/^\d{4}-\d{2}-\d{2}$/); // ISO date format
    });
  });

  describe('getDefaultShareForm', () => {
    it('should return default share form data', () => {
      const result = getDefaultShareForm();
      expect(result).toEqual({
        targetUserId: '',
        permissionLevel: 'view',
      });
    });
  });
});
