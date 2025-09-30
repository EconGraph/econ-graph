/**
 * REQUIREMENT: Professional collaboration hooks for Bloomberg Terminal-level functionality
 * PURPOSE: Provide React hooks for managing chart annotations, comments, and sharing
 * This enables institutional-grade collaboration workflows for economic analysis.
 */

import { useState, useEffect, useCallback } from 'react';
import {
  executeGraphQL,
  QUERIES,
  MUTATIONS,
  ChartAnnotationType,
  AnnotationCommentType,
  ChartCollaboratorType,
  UserType,
  CreateAnnotationInput,
  ShareChartInput,
  // DeleteAnnotationInput, // Unused import
  AnnotationsForSeriesResponse,
  CommentsForAnnotationResponse,
  ChartCollaboratorsResponse,
  UserResponse,
} from '../utils/graphql';
import { useAuth } from '../contexts/AuthContext';

export interface CollaborationState {
  annotations: ChartAnnotationType[];
  comments: Record<string, AnnotationCommentType[]>;
  collaborators: ChartCollaboratorType[];
  users: Record<string, UserType>;
  loading: boolean;
  error: string | null;
}

export interface UseCollaborationOptions {
  seriesId?: string;
  chartId?: string;
  autoRefresh?: boolean;
  refreshInterval?: number;
}

/**
 * Hook for managing chart collaboration features.
 * Provides real-time collaboration capabilities for charts and annotations.
 * @param options - Configuration options for collaboration features.
 * @returns Object containing collaboration state and methods.
 */
export function useCollaboration(options: UseCollaborationOptions = {}) {
  const { seriesId, chartId, autoRefresh = false, refreshInterval = 30000 } = options;
  const { user: currentUser } = useAuth();

  const [state, setState] = useState<CollaborationState>({
    annotations: [],
    comments: {},
    collaborators: [],
    users: {},
    loading: false,
    error: null,
  });

  // Load annotations for a series
  const loadAnnotations = useCallback(
    async (targetSeriesId?: string) => {
      if (!targetSeriesId && !seriesId) return;

      setState(prev => ({ ...prev, loading: true, error: null }));

      try {
        const response = await executeGraphQL<AnnotationsForSeriesResponse>({
          query: QUERIES.GET_ANNOTATIONS_FOR_SERIES,
          variables: {
            seriesId: targetSeriesId || seriesId,
            userId: currentUser?.id,
          },
        });

        if (response.data) {
          setState(prev => ({
            ...prev,
            annotations: response.data!.annotationsForSeries,
            loading: false,
          }));

          // Load user details for annotation authors
          const userIds = [...new Set(response.data.annotationsForSeries.map(a => a.user_id))];
          await loadUsers(userIds);
        }
      } catch (error) {
        setState(prev => ({
          ...prev,
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load annotations',
        }));
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [seriesId, currentUser]
  ); // loadUsers is called but not a dependency to avoid circular reference

  // Load comments for an annotation
  const loadComments = useCallback(async (annotationId: string) => {
    try {
      const response = await executeGraphQL<CommentsForAnnotationResponse>({
        query: QUERIES.GET_COMMENTS_FOR_ANNOTATION,
        variables: { annotationId },
      });

      if (response.data) {
        setState(prev => ({
          ...prev,
          comments: {
            ...prev.comments,
            [annotationId]: response.data!.commentsForAnnotation,
          },
        }));

        // Load user details for comment authors
        const userIds = [...new Set(response.data.commentsForAnnotation.map(c => c.user_id))];
        await loadUsers(userIds);
      }
    } catch (error) {
      console.error('Failed to load comments:', error);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []); // loadUsers is called but not a dependency to avoid circular reference

  // Load collaborators for a chart
  const loadCollaborators = useCallback(
    async (targetChartId?: string) => {
      const resolvedChartId = targetChartId || chartId;

      // Validate UUID format before making GraphQL call
      if (!resolvedChartId) return;

      const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
      if (!uuidRegex.test(resolvedChartId)) {
        console.warn('Invalid chartId format:', resolvedChartId, 'Skipping collaborators load');
        return;
      }

      try {
        const response = await executeGraphQL<ChartCollaboratorsResponse>({
          query: QUERIES.GET_CHART_COLLABORATORS,
          variables: { chartId: resolvedChartId },
        });

        if (response.data) {
          setState(prev => ({
            ...prev,
            collaborators: response.data!.chartCollaborators,
          }));

          // Load user details for collaborators
          const userIds = [...new Set(response.data.chartCollaborators.map(c => c.user_id))];
          await loadUsers(userIds);
        }
      } catch (error) {
        console.error('Failed to load collaborators:', error);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [chartId]
  ); // loadUsers is called but not a dependency to avoid circular reference

  // Load user details
  const loadUsers = useCallback(
    async (userIds: string[]) => {
      const newUserIds = userIds.filter(id => !state.users[id]);
      if (newUserIds.length === 0) return;

      try {
        const userPromises = newUserIds.map(userId =>
          executeGraphQL<UserResponse>({
            query: QUERIES.GET_USER,
            variables: { userId },
          })
        );

        const responses = await Promise.all(userPromises);
        const newUsers: Record<string, UserType> = {};

        responses.forEach((response, index) => {
          if (response.data?.user) {
            newUsers[newUserIds[index]] = response.data.user;
          }
        });

        setState(prev => ({
          ...prev,
          users: { ...prev.users, ...newUsers },
        }));
      } catch (error) {
        console.error('Failed to load users:', error);
      }
    },
    [state.users]
  );

  // Create annotation
  const createAnnotation = useCallback(
    async (input: Omit<CreateAnnotationInput, 'user_id'>) => {
      if (!currentUser) {
        throw new Error('User must be authenticated to create annotations');
      }

      setState(prev => ({ ...prev, loading: true, error: null }));

      try {
        const response = await executeGraphQL({
          query: MUTATIONS.CREATE_ANNOTATION,
          variables: {
            input: {
              ...input,
              user_id: currentUser.id,
            },
          },
        });

        if (response.data) {
          // Refresh annotations
          await loadAnnotations();
        }

        return response.data?.createAnnotation;
      } catch (error) {
        setState(prev => ({
          ...prev,
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to create annotation',
        }));
        throw error;
      }
    },
    [currentUser, loadAnnotations]
  );

  // Add comment
  const addComment = useCallback(
    async (annotationId: string, content: string) => {
      if (!currentUser) {
        throw new Error('User must be authenticated to add comments');
      }

      try {
        const response = await executeGraphQL({
          query: MUTATIONS.ADD_COMMENT,
          variables: {
            input: {
              user_id: currentUser.id,
              annotation_id: annotationId,
              content,
            },
          },
        });

        if (response.data) {
          // Refresh comments for this annotation
          await loadComments(annotationId);
        }

        return response.data?.addComment;
      } catch (error) {
        console.error('Failed to add comment:', error);
        throw error;
      }
    },
    [currentUser, loadComments]
  );

  // Share chart
  const shareChart = useCallback(
    async (input: Omit<ShareChartInput, 'owner_user_id'>) => {
      if (!currentUser) {
        throw new Error('User must be authenticated to share charts');
      }

      try {
        const response = await executeGraphQL({
          query: MUTATIONS.SHARE_CHART,
          variables: {
            input: {
              ...input,
              owner_user_id: currentUser.id,
            },
          },
        });

        if (response.data) {
          // Refresh collaborators
          await loadCollaborators();
        }

        return response.data?.shareChart;
      } catch (error) {
        console.error('Failed to share chart:', error);
        throw error;
      }
    },
    [currentUser, loadCollaborators]
  );

  // Delete annotation
  const deleteAnnotation = useCallback(
    async (annotationId: string) => {
      if (!currentUser) {
        throw new Error('User must be authenticated to delete annotations');
      }

      setState(prev => ({ ...prev, loading: true, error: null }));

      try {
        await executeGraphQL({
          query: MUTATIONS.DELETE_ANNOTATION,
          variables: {
            input: {
              user_id: currentUser.id,
              annotation_id: annotationId,
            },
          },
        });

        // Refresh annotations
        await loadAnnotations();
      } catch (error) {
        setState(prev => ({
          ...prev,
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to delete annotation',
        }));
        throw error;
      }
    },
    [currentUser, loadAnnotations]
  );

  // Toggle annotation visibility
  const toggleAnnotationVisibility = useCallback(async (annotationId: string) => {
    // For now, we'll implement this as a local state update
    // In a full implementation, this would be a backend mutation
    setState(prev => ({
      ...prev,
      annotations: prev.annotations.map(annotation =>
        annotation.id === annotationId
          ? { ...annotation, is_visible: !annotation.is_visible }
          : annotation
      ),
    }));
  }, []);

  // Toggle annotation pin status
  const toggleAnnotationPin = useCallback(async (annotationId: string) => {
    // For now, we'll implement this as a local state update
    // In a full implementation, this would be a backend mutation
    setState(prev => ({
      ...prev,
      annotations: prev.annotations.map(annotation =>
        annotation.id === annotationId
          ? { ...annotation, is_pinned: !annotation.is_pinned }
          : annotation
      ),
    }));
  }, []);

  // Load initial data
  useEffect(() => {
    if (seriesId) {
      loadAnnotations();
    }
  }, [seriesId, loadAnnotations]);

  useEffect(() => {
    if (chartId) {
      loadCollaborators();
    }
  }, [chartId, loadCollaborators]);

  // Auto-refresh
  useEffect(() => {
    if (!autoRefresh) return;

    const interval = setInterval(() => {
      if (seriesId) {
        loadAnnotations();
      }
      if (chartId) {
        loadCollaborators();
      }
    }, refreshInterval);

    return () => clearInterval(interval);
  }, [autoRefresh, refreshInterval, seriesId, chartId, loadAnnotations, loadCollaborators]);

  return {
    // State
    ...state,

    // Actions
    createAnnotation,
    addComment,
    shareChart,
    deleteAnnotation,
    toggleAnnotationVisibility,
    toggleAnnotationPin,

    // Loading actions
    loadAnnotations,
    loadComments,
    loadCollaborators,
    loadUsers,

    // Utilities
    getUserById: (userId: string) => state.users[userId],
    getCommentsForAnnotation: (annotationId: string) => state.comments[annotationId] || [],
  };
}
