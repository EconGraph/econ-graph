import React from 'react';
import {
  Alert,
  Box,
  Button,
  List,
  ListItem,
  ListItemText,
  Skeleton,
  TextField,
  Typography,
} from '@mui/material';
import { useAddComment, useAnnotationComments } from '../../hooks/useAnnotationEditing';
import { charCount, COMMENT_MAX_CHARS, wrapAnywhere } from './annotationLimits';

export interface AnnotationCommentsProps {
  /** Element id, for the toggle's `aria-controls`. */
  id?: string;
  /** Annotation title, to name the thread for screen readers. */
  title: string;
  annotationId: string;
  /** Signed-in user's id, to label their own comments; null when signed out. */
  viewerId: string | null;
  /** Show the form to add a comment. */
  canComment: boolean;
}

const formatTimestamp = (value: string | null) =>
  value
    ? new Date(value).toLocaleString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
      })
    : '';

/**
 * The comment thread under one annotation, with a form to add a comment when signed in.
 * Comments load when the thread is first shown.
 * @param props - See `AnnotationCommentsProps`.
 * @param props.id - Element id.
 * @param props.title - Annotation title.
 * @param props.annotationId - Annotation whose comments to show.
 * @param props.viewerId - Signed-in user's id, or null.
 * @param props.canComment - Show the form to add a comment.
 * @returns The thread.
 */
const AnnotationComments: React.FC<AnnotationCommentsProps> = ({
  id,
  title,
  annotationId,
  viewerId,
  canComment,
}) => {
  const comments = useAnnotationComments(annotationId);
  const addComment = useAddComment(annotationId);
  const [draft, setDraft] = React.useState('');
  // Counted in code points, as the backend counts them; an input `maxLength` would count UTF-16
  // units, so an emoji would use up two and pasted text would be cut without a word.
  const draftChars = charCount(draft.trim());
  const draftTooLong = draftChars > COMMENT_MAX_CHARS;

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    const content = draft.trim();
    if (!content || draftTooLong || addComment.isPending) return;
    addComment.mutate(content, { onSuccess: () => setDraft('') });
  };

  let body: React.ReactNode;
  if (comments.isPending) {
    body = <Skeleton variant='rectangular' height={32} data-testid='comments-loading' />;
  } else if (comments.isError) {
    body = (
      <Alert
        severity='error'
        action={
          <Button color='inherit' size='small' onClick={() => comments.refetch()}>
            Retry
          </Button>
        }
      >
        Could not load comments. {(comments.error as Error | null)?.message}
      </Alert>
    );
  } else if (!comments.data?.length) {
    body = (
      <Typography variant='body2' color='text.secondary'>
        No comments yet.
      </Typography>
    );
  } else {
    body = (
      <List dense disablePadding aria-label={`Comments on ${title}`}>
        {comments.data.map(comment => (
          <ListItem key={comment.id} disableGutters>
            <ListItemText
              primary={comment.content}
              primaryTypographyProps={{ sx: { whiteSpace: 'pre-wrap', ...wrapAnywhere } }}
              secondary={`${comment.userId === viewerId ? 'You' : 'Another user'} · ${formatTimestamp(comment.createdAt)}`}
            />
          </ListItem>
        ))}
      </List>
    );
  }

  return (
    <Box id={id} sx={{ pl: 2, pr: 1, pb: 1 }}>
      {body}
      {canComment && (
        <Box component='form' onSubmit={submit} sx={{ mt: 1 }}>
          {addComment.isError && (
            <Alert severity='error' sx={{ mb: 1 }}>
              Could not post the comment. {(addComment.error as Error | null)?.message}
            </Alert>
          )}
          <TextField
            id={`comment-draft-${annotationId}`}
            label='Add a comment'
            inputProps={{ 'aria-label': `Add a comment on ${title}` }}
            value={draft}
            error={draftTooLong}
            helperText={
              draftTooLong
                ? `${draftChars.toLocaleString()} / ${COMMENT_MAX_CHARS.toLocaleString()}: shorten the comment to post`
                : undefined
            }
            onChange={e => setDraft(e.target.value)}
            size='small'
            fullWidth
            multiline
            maxRows={4}
          />
          <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: 1 }}>
            <Button
              type='submit'
              size='small'
              variant='contained'
              disabled={!draft.trim() || draftTooLong || addComment.isPending}
              aria-label={`${addComment.isPending ? 'Posting' : 'Post comment'} on ${title}`}
            >
              {addComment.isPending ? 'Posting…' : 'Post comment'}
            </Button>
          </Box>
        </Box>
      )}
    </Box>
  );
};

export default AnnotationComments;
