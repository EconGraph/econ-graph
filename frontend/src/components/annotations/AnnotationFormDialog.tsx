import React from 'react';
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  FormHelperText,
  Stack,
  Switch,
  TextField,
} from '@mui/material';
import {
  ANNOTATION_NOTE_MAX_CHARS,
  ANNOTATION_TITLE_MAX_CHARS,
  charCount,
} from './annotationLimits';

/** What the form edits. */
export interface AnnotationFormValues {
  /** `YYYY-MM-DD`. */
  date: string;
  title: string;
  description: string;
  isPublic: boolean;
}

export interface AnnotationFormDialogProps {
  open: boolean;
  /** Create asks for a date; edit shows it read-only, since an annotation's date is fixed. */
  mode: 'create' | 'edit';
  /** Values the form starts with. Give the dialog a new `key` to start over. */
  initialValues: AnnotationFormValues;
  /** True while the save is in flight. */
  saving?: boolean;
  /** Message from a failed save. */
  error?: string | null;
  onSubmit: (values: AnnotationFormValues) => void;
  onClose: () => void;
}

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * Dialog to create or edit an annotation: date, title, note and a public/private switch.
 * @param props - See `AnnotationFormDialogProps`.
 * @param props.open - Whether the dialog is shown.
 * @param props.mode - Create or edit.
 * @param props.initialValues - Values the form starts with.
 * @param props.saving - True while the save is in flight.
 * @param props.error - Message from a failed save.
 * @param props.onSubmit - Called with the trimmed values.
 * @param props.onClose - Called when the dialog is dismissed.
 * @returns The dialog.
 */
const AnnotationFormDialog: React.FC<AnnotationFormDialogProps> = ({
  open,
  mode,
  initialValues,
  saving = false,
  error = null,
  onSubmit,
  onClose,
}) => {
  const [values, setValues] = React.useState(initialValues);

  // Counted in characters, as the server counts them (an emoji is one, not two). An annotation
  // saved before the limits existed can be over them; say so rather than fail on save. No input
  // `maxLength`: it counts UTF-16 units, so it would stop at half as many emoji and cut pasted
  // text without a word.
  const titleChars = charCount(values.title.trim());
  const noteChars = charCount(values.description.trim());
  const titleTooLong = titleChars > ANNOTATION_TITLE_MAX_CHARS;
  const noteTooLong = noteChars > ANNOTATION_NOTE_MAX_CHARS;
  const valid =
    ISO_DATE.test(values.date) && values.title.trim() !== '' && !titleTooLong && !noteTooLong;

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!valid || saving) return;
    onSubmit({ ...values, title: values.title.trim(), description: values.description.trim() });
  };

  const titleId = `annotation-form-${mode}-title`;

  return (
    <Dialog open={open} onClose={saving ? undefined : onClose} aria-labelledby={titleId} fullWidth>
      <form onSubmit={submit} noValidate>
        <DialogTitle id={titleId}>
          {mode === 'create' ? 'Add annotation' : 'Edit annotation'}
        </DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            {error && <Alert severity='error'>{error}</Alert>}
            <TextField
              label='Date'
              type='date'
              value={values.date}
              onChange={e => setValues({ ...values, date: e.target.value })}
              disabled={mode === 'edit'}
              helperText={mode === 'edit' ? "An annotation's date can't be changed." : undefined}
              required
              slotProps={{
                inputLabel: { shrink: true },
              }}
            />
            <TextField
              id={`annotation-form-${mode}-title-field`}
              label='Title'
              value={values.title}
              onChange={e => setValues({ ...values, title: e.target.value })}
              required
              error={titleTooLong}
              helperText={
                titleTooLong
                  ? `${titleChars.toLocaleString()} / ${ANNOTATION_TITLE_MAX_CHARS.toLocaleString()}: shorten the title to save`
                  : undefined
              }
            />
            <TextField
              id={`annotation-form-${mode}-note-field`}
              label='Note'
              value={values.description}
              onChange={e => setValues({ ...values, description: e.target.value })}
              multiline
              minRows={3}
              error={noteTooLong}
              helperText={`${noteChars.toLocaleString()} / ${ANNOTATION_NOTE_MAX_CHARS.toLocaleString()}${
                noteTooLong ? ': shorten the note to save' : ''
              }`}
            />
            <div>
              <FormControlLabel
                control={
                  <Switch
                    checked={values.isPublic}
                    onChange={e => setValues({ ...values, isPublic: e.target.checked })}
                  />
                }
                label='Public'
              />
              <FormHelperText>
                {values.isPublic
                  ? 'Everyone who opens this series can see it and comment.'
                  : 'Only you can see it.'}
              </FormHelperText>
            </div>
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose} disabled={saving}>
            Cancel
          </Button>
          <Button type='submit' variant='contained' disabled={!valid || saving}>
            {saving ? 'Saving…' : 'Save'}
          </Button>
        </DialogActions>
      </form>
    </Dialog>
  );
};

export default AnnotationFormDialog;
