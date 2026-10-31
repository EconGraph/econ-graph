import React, { useState } from 'react';
import { Alert, Box, Button, Stack, TextField, Typography } from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useAuth } from '../../contexts/AuthContext';
import {
  companyNoteAnchor,
  createCompanyNote,
  deleteCompanyNote,
  getCompanyNotes,
  updateCompanyNote,
} from '../../graphql/companyNotes';
import {
  charCount,
  ANNOTATION_NOTE_MAX_CHARS,
  wrapAnywhere,
} from '../annotations/annotationLimits';

export interface CompanyNotesProps {
  cik: string;
  /** A versioned economic/fact/filing anchor supplied by the authorized data API. */
  anchor?: string;
  targetLabel?: string;
}

/**
 * Keep personal notes attached to their original research targets.
 * @param props - Company and optional fact target to annotate.
 * @returns Account-scoped research notes or a sign-in prompt.
 */
export default function CompanyNotes(props: CompanyNotesProps) {
  const { user, isAuthenticated } = useAuth();
  if (!isAuthenticated || !user)
    return <Typography>Sign in to keep private company notes.</Typography>;
  return (
    <SignedInCompanyNotes
      key={`${user.id}:${props.cik}:${props.anchor ?? ''}`}
      {...props}
      userId={user.id}
    />
  );
}

function SignedInCompanyNotes({
  cik,
  anchor,
  targetLabel,
  userId,
}: CompanyNotesProps & { userId: string }) {
  const client = useQueryClient();
  // User ID keeps one account's notes out of another account's cached view.
  const [offset, setOffset] = useState(0);
  const key = ['company-notes', userId, cik, anchor ?? null, offset];
  const notes = useQuery({
    queryKey: key,
    queryFn: () => getCompanyNotes(cik, anchor, offset),
  });
  const [draft, setDraft] = useState('');
  const [editing, setEditing] = useState<string | null>(null);
  const refresh = () => client.invalidateQueries({ queryKey: ['company-notes', userId, cik] });
  const save = useMutation({
    mutationFn: ({ id, content }: { id: string | null; content: string }) =>
      id
        ? updateCompanyNote(id, content)
        : createCompanyNote(anchor ?? companyNoteAnchor(cik), content),
    onSuccess: () => {
      setDraft('');
      setEditing(null);
      void refresh();
    },
  });
  const remove = useMutation({ mutationFn: deleteCompanyNote, onSuccess: refresh });
  const busy = save.isPending || remove.isPending;
  const invalid = draft.trim().length === 0 || charCount(draft) > ANNOTATION_NOTE_MAX_CHARS;

  return (
    <Box component='section' aria-label='My company notes' sx={{ mt: 3 }}>
      <Typography variant='h6'>My company notes</Typography>
      <Typography variant='body2' color='text.secondary'>
        Private to your account. Notes keep their original target when filings are revised.
      </Typography>
      {targetLabel && <Typography variant='body2'>Target: {targetLabel}</Typography>}
      {notes.isPending && <Typography role='status'>Loading notes…</Typography>}
      {notes.error && <Alert severity='error'>{notes.error.message}</Alert>}
      {(save.error || remove.error) && (
        <Alert severity='error'>{save.error?.message ?? remove.error?.message}</Alert>
      )}
      <Stack spacing={2} sx={{ my: 2 }}>
        {notes.data?.map(note => (
          <Box key={note.id} sx={{ border: 1, borderColor: 'divider', p: 2, ...wrapAnywhere }}>
            <Typography sx={{ whiteSpace: 'pre-wrap' }}>{note.content}</Typography>
            {note.targetStatus !== 'found' && (
              <Typography variant='body2'>
                Original target is currently unavailable. Your note is preserved.
              </Typography>
            )}
            <Typography variant='caption'>
              Updated {new Date(note.updatedAt).toLocaleString()}
            </Typography>
            <Stack direction='row' spacing={1}>
              <Button
                disabled={busy}
                onClick={() => {
                  setEditing(note.id);
                  setDraft(note.content);
                  save.reset();
                }}
              >
                Edit note
              </Button>
              <Button disabled={busy} onClick={() => remove.mutate(note.id)}>
                Delete note
              </Button>
            </Stack>
          </Box>
        ))}
        {notes.data?.length === 0 && <Typography>No notes for this target yet.</Typography>}
      </Stack>
      <Stack direction='row' spacing={1} sx={{ mb: 2 }}>
        {offset > 0 && (
          <Button onClick={() => setOffset(Math.max(0, offset - 100))}>Newer notes</Button>
        )}
        {notes.data?.length === 100 && (
          <Button onClick={() => setOffset(offset + 100)}>Older notes</Button>
        )}
      </Stack>
      <Box
        component='form'
        onSubmit={event => {
          event.preventDefault();
          if (!invalid && !busy) save.mutate({ id: editing, content: draft });
        }}
      >
        <TextField
          fullWidth
          multiline
          minRows={3}
          label={editing ? 'Edit private note' : 'New private note'}
          value={draft}
          onChange={event => setDraft(event.target.value)}
          disabled={busy}
          error={charCount(draft) > ANNOTATION_NOTE_MAX_CHARS}
          helperText={`${charCount(draft)} / ${ANNOTATION_NOTE_MAX_CHARS} characters`}
        />
        <Button type='submit' disabled={invalid || busy}>
          {save.isPending ? 'Saving…' : editing ? 'Save changes' : 'Save note'}
        </Button>
        {editing && (
          <Button
            disabled={busy}
            onClick={() => {
              setEditing(null);
              setDraft('');
            }}
          >
            Cancel edit
          </Button>
        )}
      </Box>
    </Box>
  );
}
