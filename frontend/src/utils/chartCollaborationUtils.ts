/**
 * REQUIREMENT: Pure business logic utilities for chart collaboration
 * PURPOSE: Extract testable business logic from ChartCollaborationConnected component
 * This enables unit testing of core functionality without UI dependencies
 */

import { ChartAnnotationType } from './graphql';

export interface Collaborator {
  id: string;
  userId: string;
  chartId: string;
  role: string;
  lastAccessedAt?: string;
  createdAt: string;
  updatedAt: string;
}

export interface User {
  id: string;
  name: string;
  email: string;
  avatarUrl?: string;
}

export type AnnotationFilter = 'all' | 'mine' | 'pinned';

/**
 * Filter annotations based on filter type and current user
 * @param annotations - Array of annotations to filter
 * @param filterBy - Filter type: 'all', 'mine', or 'pinned'
 * @param currentUserId - Current user ID for 'mine' filter
 * @returns Filtered array of annotations
 */
export const filterAnnotations = (
  annotations: ChartAnnotationType[],
  filterBy: AnnotationFilter,
  currentUserId?: string
): ChartAnnotationType[] => {
  switch (filterBy) {
    case 'mine':
      return annotations.filter(annotation => annotation.userId === currentUserId);
    case 'pinned':
      return annotations.filter(annotation => annotation.isPinned);
    default:
      return annotations.filter(annotation => annotation.isVisible !== false);
  }
};

/**
 * Calculate total number of comments across all annotations
 * @param annotations - Array of annotations
 * @param getCommentsForAnnotation - Function to get comments for a specific annotation
 * @returns Total number of comments
 */
export const calculateTotalComments = (
  annotations: ChartAnnotationType[],
  getCommentsForAnnotation: (annotationId: string) => any[] | undefined
): number => {
  return annotations.reduce((sum, annotation) => {
    const comments = getCommentsForAnnotation(annotation.id);
    return sum + (comments?.length || 0);
  }, 0);
};

/**
 * Filter active collaborators based on last access time
 * @param collaborators - Array of collaborators
 * @param users - Map of user data
 * @param activeThresholdMs - Threshold in milliseconds for considering a collaborator active (default: 5 minutes)
 * @returns Array of active collaborators
 */
export const filterActiveCollaborators = (
  collaborators: Collaborator[],
  users: Record<string, User>,
  activeThresholdMs: number = 300000 // 5 minutes
): Collaborator[] => {
  const now = Date.now();
  return collaborators.filter(collaborator => {
    const user = users[collaborator.userId];
    if (!user) return false;

    const lastAccessed = new Date(collaborator.lastAccessedAt || 0).getTime();
    return now - lastAccessed < activeThresholdMs;
  });
};

/**
 * Validate annotation form data
 * @param formData - Annotation form data
 * @returns Validation result with errors
 */
export const validateAnnotationForm = (formData: {
  title: string;
  content: string;
  annotationDate: string;
  annotationValue?: string;
  annotationType: string;
}): { isValid: boolean; errors: string[] } => {
  const errors: string[] = [];

  if (!formData.title.trim()) {
    errors.push('Title is required');
  }

  if (!formData.content.trim()) {
    errors.push('Content is required');
  }

  if (!formData.annotationDate) {
    errors.push('Date is required');
  }

  if (formData.annotationValue && isNaN(parseFloat(formData.annotationValue))) {
    errors.push('Value must be a valid number');
  }

  if (!formData.annotationType) {
    errors.push('Annotation type is required');
  }

  return {
    isValid: errors.length === 0,
    errors,
  };
};

/**
 * Validate share form data
 * @param formData - Share form data
 * @param chartId - Chart ID
 * @returns Validation result with errors
 */
export const validateShareForm = (
  formData: {
    targetUserId: string;
    permissionLevel: string;
  },
  chartId?: string
): { isValid: boolean; errors: string[] } => {
  const errors: string[] = [];

  if (!chartId) {
    errors.push('Chart ID is required');
  }

  if (!formData.targetUserId) {
    errors.push('Target user is required');
  }

  if (!formData.permissionLevel) {
    errors.push('Permission level is required');
  }

  return {
    isValid: errors.length === 0,
    errors,
  };
};

/**
 * Format annotation data for API submission
 * @param formData - Annotation form data
 * @param seriesId - Series ID
 * @returns Formatted data for API
 */
export const formatAnnotationForApi = (
  formData: {
    title: string;
    content: string;
    annotationDate: string;
    annotationValue?: string;
    annotationType: string;
    color: string;
    isPublic: boolean;
  },
  seriesId: string
) => {
  return {
    series_id: seriesId,
    annotation_date: formData.annotationDate,
    annotation_value: formData.annotationValue ? parseFloat(formData.annotationValue) : undefined,
    title: formData.title,
    content: formData.content,
    annotation_type: formData.annotationType,
    color: formData.color,
    is_public: formData.isPublic,
  };
};

/**
 * Get default annotation form state
 * @returns Default annotation form data
 */
export const getDefaultAnnotationForm = () => ({
  annotationDate: new Date().toISOString().split('T')[0],
  annotationValue: '',
  title: '',
  content: '',
  color: '#f44336',
  annotationType: 'note',
  isPublic: true,
});

/**
 * Get default share form state
 * @returns Default share form data
 */
export const getDefaultShareForm = () => ({
  targetUserId: '',
  permissionLevel: 'view',
});
