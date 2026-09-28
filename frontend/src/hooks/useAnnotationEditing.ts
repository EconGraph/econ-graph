/**
 * Signed-in annotation editing on the series page: create, update and delete annotations, and
 * read and add comments. The author is never sent; the backend takes it from the access token
 * that `executeGraphQL` attaches for a signed-in user.
 */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AddCommentInput,
  AddCommentResponse,
  AnnotationCommentType,
  CommentsForAnnotationResponse,
  CreateAnnotationInput,
  CreateAnnotationResponse,
  DeleteAnnotationResponse,
  executeGraphQL,
  MUTATIONS,
  QUERIES,
  UpdateAnnotationInput,
  UpdateAnnotationResponse,
} from '../utils/graphql';
import { seriesAnnotationsKey } from './useSeriesAnnotations';

/** Annotation type the series page creates; the backend requires one. */
export const SERIES_ANNOTATION_TYPE = 'note';

/**
 * React-query key of an annotation's comments.
 * @param annotationId - Annotation id.
 * @returns The key.
 */
export const annotationCommentsKey = (annotationId: string) =>
  ['annotationComments', annotationId] as const;

/** What the create form collects; the series and type are filled in here. */
export type NewSeriesAnnotation = Omit<CreateAnnotationInput, 'seriesId' | 'annotationType'>;

/**
 * Create, update and delete annotations on one series. Each refetches the series'
 * annotations when it succeeds.
 * @param seriesId - Series id (UUID).
 * @returns One react-query mutation per operation.
 */
export function useAnnotationMutations(seriesId: string) {
  const queryClient = useQueryClient();
  const refresh = () => queryClient.invalidateQueries(seriesAnnotationsKey(seriesId));

  const create = useMutation(
    async (input: NewSeriesAnnotation) => {
      const payload: CreateAnnotationInput = {
        ...input,
        seriesId,
        annotationType: SERIES_ANNOTATION_TYPE,
      };
      const result = await executeGraphQL<CreateAnnotationResponse>({
        query: MUTATIONS.CREATE_ANNOTATION,
        variables: { input: payload },
      });
      return result.data?.createAnnotation;
    },
    { onSuccess: refresh }
  );

  const update = useMutation(
    async (input: UpdateAnnotationInput) => {
      const result = await executeGraphQL<UpdateAnnotationResponse>({
        query: MUTATIONS.UPDATE_ANNOTATION,
        variables: { input },
      });
      return result.data?.updateAnnotation;
    },
    { onSuccess: refresh }
  );

  const remove = useMutation(
    async (annotationId: string) => {
      const result = await executeGraphQL<DeleteAnnotationResponse>({
        query: MUTATIONS.DELETE_ANNOTATION,
        variables: { input: { annotationId } },
      });
      return result.data?.deleteAnnotation ?? false;
    },
    { onSuccess: refresh }
  );

  return { create, update, remove };
}

/**
 * Load the comments on one annotation, oldest first (the backend's order).
 * @param annotationId - Annotation id.
 * @returns The react-query result.
 */
export function useAnnotationComments(annotationId: string) {
  return useQuery(
    annotationCommentsKey(annotationId),
    async (): Promise<AnnotationCommentType[]> => {
      const result = await executeGraphQL<CommentsForAnnotationResponse>({
        query: QUERIES.GET_COMMENTS_FOR_ANNOTATION,
        variables: { annotationId },
      });
      return result.data?.commentsForAnnotation ?? [];
    }
  );
}

/**
 * Add a comment to an annotation, then refetch its comments.
 * @param annotationId - Annotation id.
 * @returns The react-query mutation; call it with the comment text.
 */
export function useAddComment(annotationId: string) {
  const queryClient = useQueryClient();
  return useMutation(
    async (content: string) => {
      const input: AddCommentInput = { annotationId, content };
      const result = await executeGraphQL<AddCommentResponse>({
        query: MUTATIONS.ADD_COMMENT,
        variables: { input },
      });
      return result.data?.addComment;
    },
    { onSuccess: () => queryClient.invalidateQueries(annotationCommentsKey(annotationId)) }
  );
}
