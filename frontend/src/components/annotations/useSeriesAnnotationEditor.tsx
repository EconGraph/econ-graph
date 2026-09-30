/**
 * Signed-in annotation controls for the series page. The hook returns what plugs into
 * UI-8's `SeriesAnnotationsPanel` (`headerAction`, `renderAnnotationExtra`) and UI-5's
 * `SeriesChart` (`onPointClick`), plus the dialogs to render once on the page.
 */

import React from 'react';
import {
  Alert,
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  IconButton,
  Tooltip,
} from '@mui/material';
import {
  Add as AddIcon,
  ChatBubbleOutline as CommentsIcon,
  Delete as DeleteIcon,
  Edit as EditIcon,
  Lock as LockIcon,
} from '@mui/icons-material';
import { useAuth } from '../../contexts/AuthContext';
import { useAnnotationMutations } from '../../hooks/useAnnotationEditing';
import type { SeriesAnnotation } from '../../hooks/useSeriesAnnotations';
import type { SeriesChartPoint } from '../charts/SeriesChart';
import AnnotationComments from './AnnotationComments';
import AnnotationFormDialog, { AnnotationFormValues } from './AnnotationFormDialog';

export interface SeriesAnnotationEditorOptions {
  seriesId: string;
  /** Date the create form starts on when opened from the header button, `YYYY-MM-DD`. */
  defaultDate: string;
  /** Called with a newly created annotation's id, for example to select it. */
  onCreated?: (id: string) => void;
  /** Called with a deleted annotation's id, for example to clear the selection. */
  onDeleted?: (id: string) => void;
}

export interface SeriesAnnotationEditor {
  /** For `SeriesAnnotationsPanel.headerAction`. */
  headerAction: React.ReactNode;
  /** For `SeriesAnnotationsPanel.renderAnnotationExtra`. */
  renderAnnotationExtra: (annotation: SeriesAnnotation) => React.ReactNode;
  /** For `SeriesChart.onPointClick`; undefined when the viewer can't annotate. */
  onPointClick?: (point: SeriesChartPoint) => void;
  /** The create, edit and delete dialogs. */
  dialogs: React.ReactNode;
}

type FormState =
  | { mode: 'create'; values: AnnotationFormValues }
  | { mode: 'edit'; id: string; values: AnnotationFormValues };

const errorText = (error: unknown) =>
  error instanceof Error && error.message ? error.message : 'Something went wrong.';

/**
 * Annotation create, edit, delete and comment controls for one series. Everything that
 * writes is shown only to a signed-in user, and only when sign-in is configured
 * (`VITE_OIDC_ISSUER`). Edit and delete are shown only on the user's own annotations; the
 * backend enforces the same.
 * @param options - See `SeriesAnnotationEditorOptions`.
 * @param options.seriesId - Series id (UUID).
 * @param options.defaultDate - Date the create form starts on from the header button.
 * @param options.onCreated - Called with a new annotation's id.
 * @param options.onDeleted - Called with a deleted annotation's id.
 * @returns The panel and chart props and the dialogs.
 */
export function useSeriesAnnotationEditor({
  seriesId,
  defaultDate,
  onCreated,
  onDeleted,
}: SeriesAnnotationEditorOptions): SeriesAnnotationEditor {
  const { isConfigured, isAuthenticated, isLoading, user, signIn } = useAuth();
  const viewerId = isConfigured && isAuthenticated && user ? user.id : null;
  const canWrite = viewerId !== null;

  const { create, update, remove } = useAnnotationMutations(seriesId);
  // The last form opened stays set while the dialog closes, so it doesn't change mode or
  // values during the exit transition.
  const [form, setForm] = React.useState<FormState | null>(null);
  const [formOpen, setFormOpen] = React.useState(false);
  // Remounts the form dialog on each open, so it starts from that open's values.
  const [formSession, setFormSession] = React.useState(0);
  // As with the form, the last annotation asked about stays set while the dialog closes.
  const [deleting, setDeleting] = React.useState<SeriesAnnotation | null>(null);
  const [deleteOpen, setDeleteOpen] = React.useState(false);
  const [openThreads, setOpenThreads] = React.useState<ReadonlySet<string>>(new Set());

  const { reset: resetCreate } = create;
  const openCreate = React.useCallback(
    (date: string) => {
      resetCreate();
      setFormSession(n => n + 1);
      setForm({ mode: 'create', values: { date, title: '', description: '', isPublic: false } });
      setFormOpen(true);
    },
    [resetCreate]
  );

  const onPointClick = React.useMemo(
    () => (canWrite ? (point: SeriesChartPoint) => openCreate(point.date) : undefined),
    [canWrite, openCreate]
  );

  const openEdit = (annotation: SeriesAnnotation) => {
    update.reset();
    setFormSession(n => n + 1);
    setForm({
      mode: 'edit',
      id: annotation.id,
      values: {
        date: annotation.date,
        title: annotation.title,
        description: annotation.description ?? '',
        isPublic: annotation.visibility === 'PUBLIC',
      },
    });
    setFormOpen(true);
  };

  const openDelete = (annotation: SeriesAnnotation) => {
    remove.reset();
    setDeleting(annotation);
    setDeleteOpen(true);
  };

  const toggleThread = (id: string) =>
    setOpenThreads(current => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const submitForm = (values: AnnotationFormValues) => {
    if (!form) return;
    if (form.mode === 'create') {
      create.mutate(
        {
          annotationDate: values.date,
          title: values.title,
          content: values.description,
          isPublic: values.isPublic,
        },
        {
          onSuccess: created => {
            setFormOpen(false);
            if (created) onCreated?.(created.id);
          },
        }
      );
    } else {
      update.mutate(
        {
          annotationId: form.id,
          title: values.title,
          content: values.description,
          isPublic: values.isPublic,
        },
        { onSuccess: () => setFormOpen(false) }
      );
    }
  };

  const confirmDelete = () => {
    if (!deleting) return;
    const { id } = deleting;
    remove.mutate(id, {
      onSuccess: () => {
        setDeleteOpen(false);
        onDeleted?.(id);
      },
    });
  };

  let headerAction: React.ReactNode = null;
  if (canWrite) {
    headerAction = (
      <Button size='small' startIcon={<AddIcon />} onClick={() => openCreate(defaultDate)}>
        Add annotation
      </Button>
    );
  } else if (isConfigured && !isAuthenticated && !isLoading) {
    headerAction = (
      <Button size='small' onClick={() => void signIn()}>
        Sign in to annotate
      </Button>
    );
  }

  const renderAnnotationExtra = (annotation: SeriesAnnotation) => {
    const isAuthor = viewerId !== null && annotation.authorId === viewerId;
    const threadOpen = openThreads.has(annotation.id);
    return (
      <Box>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5, pl: 2, pr: 1 }}>
          {/* Only the author is ever sent a private annotation, but check anyway. */}
          {isAuthor && annotation.visibility === 'PRIVATE' && (
            <Chip
              icon={<LockIcon />}
              label='Private'
              size='small'
              variant='outlined'
              data-testid='private-badge'
            />
          )}
          <Button
            size='small'
            startIcon={<CommentsIcon />}
            aria-label={`${threadOpen ? 'Hide comments' : 'Comments'} on ${annotation.title}`}
            aria-expanded={threadOpen}
            aria-controls={threadOpen ? `annotation-comments-${annotation.id}` : undefined}
            onClick={() => toggleThread(annotation.id)}
          >
            {threadOpen ? 'Hide comments' : 'Comments'}
          </Button>
          {isAuthor && (
            <Box sx={{ ml: 'auto' }}>
              <Tooltip title='Edit'>
                <IconButton
                  size='small'
                  aria-label={`Edit ${annotation.title}`}
                  onClick={() => openEdit(annotation)}
                >
                  <EditIcon fontSize='small' />
                </IconButton>
              </Tooltip>
              <Tooltip title='Delete'>
                <IconButton
                  size='small'
                  aria-label={`Delete ${annotation.title}`}
                  onClick={() => openDelete(annotation)}
                >
                  <DeleteIcon fontSize='small' />
                </IconButton>
              </Tooltip>
            </Box>
          )}
        </Box>
        {threadOpen && (
          <AnnotationComments
            id={`annotation-comments-${annotation.id}`}
            title={annotation.title}
            annotationId={annotation.id}
            viewerId={viewerId}
            canComment={canWrite}
          />
        )}
      </Box>
    );
  };

  const activeMutation = form?.mode === 'edit' ? update : create;
  const dialogs = canWrite ? (
    <>
      <AnnotationFormDialog
        key={formSession}
        open={formOpen}
        mode={form?.mode ?? 'create'}
        initialValues={
          form?.values ?? { date: defaultDate, title: '', description: '', isPublic: false }
        }
        saving={activeMutation.isLoading}
        error={activeMutation.isError ? errorText(activeMutation.error) : null}
        onSubmit={submitForm}
        onClose={() => setFormOpen(false)}
      />
      <Dialog
        open={deleteOpen}
        onClose={remove.isLoading ? undefined : () => setDeleteOpen(false)}
        aria-labelledby='delete-annotation-title'
      >
        <DialogTitle id='delete-annotation-title'>Delete annotation?</DialogTitle>
        <DialogContent>
          {remove.isError && (
            <Alert severity='error' sx={{ mb: 2 }}>
              {errorText(remove.error)}
            </Alert>
          )}
          <DialogContentText>
            &ldquo;{deleting?.title}&rdquo; and its comments will be deleted for everyone.
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDeleteOpen(false)} disabled={remove.isLoading}>
            Cancel
          </Button>
          <Button color='error' onClick={confirmDelete} disabled={remove.isLoading}>
            {remove.isLoading ? 'Deleting…' : 'Delete'}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  ) : null;

  return { headerAction, renderAnnotationExtra, onPointClick, dialogs };
}
