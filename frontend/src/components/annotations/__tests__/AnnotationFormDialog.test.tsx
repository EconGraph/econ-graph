/**
 * REQUIREMENT: An annotation saved before the length limits existed can still be opened, and the
 * dialog says what is too long instead of failing on save.
 * PURPOSE: Regression tests for ECO-402's limits in the edit dialog.
 */

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';

import AnnotationFormDialog from '../AnnotationFormDialog';
import { ANNOTATION_NOTE_MAX_CHARS, ANNOTATION_TITLE_MAX_CHARS } from '../annotationLimits';

const renderDialog = (title: string, description = '') =>
  render(
    <AnnotationFormDialog
      open
      mode='edit'
      initialValues={{ date: '2024-01-01', title, description, isPublic: true }}
      onSubmit={vi.fn()}
      onClose={vi.fn()}
    />
  );

describe('AnnotationFormDialog limits', () => {
  it('saves a title and note within the limits', () => {
    renderDialog('t'.repeat(ANNOTATION_TITLE_MAX_CHARS), 'n'.repeat(ANNOTATION_NOTE_MAX_CHARS));
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
  });

  it('flags an over-long title and blocks saving', () => {
    renderDialog('t'.repeat(ANNOTATION_TITLE_MAX_CHARS + 30));
    expect(screen.getByText(/shorten the title to save/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  });

  it('flags an over-long note and blocks saving', () => {
    renderDialog('Title', 'n'.repeat(ANNOTATION_NOTE_MAX_CHARS + 1));
    expect(screen.getByText(/shorten the note to save/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
  });

  it('takes a full-length title of emoji, which are two UTF-16 units each', () => {
    // An input `maxLength` counts UTF-16 units and would cut this to 100 emoji (CodeRabbit on #403).
    renderDialog('Title');
    const emoji = '📈'.repeat(ANNOTATION_TITLE_MAX_CHARS);
    const title = screen.getByRole('textbox', { name: /Title/ });
    expect(title).not.toHaveAttribute('maxlength');
    fireEvent.change(title, { target: { value: emoji } });
    expect(title).toHaveValue(emoji);
    expect(screen.queryByText(/shorten the title to save/)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled();
  });

  it('has no UTF-16 maxLength on the note either', () => {
    renderDialog('Title');
    expect(screen.getByRole('textbox', { name: /Note/ })).not.toHaveAttribute('maxlength');
  });
});
