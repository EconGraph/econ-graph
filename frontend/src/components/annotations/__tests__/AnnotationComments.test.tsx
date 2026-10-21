/**
 * REQUIREMENT: A comment can be as long as the backend allows, counted the way the backend counts
 * (code points), and an over-long one is flagged rather than silently cut.
 * PURPOSE: Regression tests for ECO-402's comment limit (CodeRabbit on #403: an input
 * `maxLength` counted emoji twice).
 */

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';

import AnnotationComments from '../AnnotationComments';
import { COMMENT_MAX_CHARS } from '../annotationLimits';

vi.mock('../../../hooks/useAnnotationEditing', () => ({
  useAnnotationComments: () => ({ isLoading: false, isError: false, data: [] }),
  useAddComment: () => ({ mutate: vi.fn(), isLoading: false, isError: false, error: null }),
}));

const renderComments = () =>
  render(
    <AnnotationComments title='Recession' annotationId='a1' viewerId='u1' canComment />
  );

describe('AnnotationComments limit', () => {
  it('takes a full-length comment of emoji', () => {
    renderComments();
    const box = screen.getByRole('textbox', { name: 'Add a comment on Recession' });
    expect(box).not.toHaveAttribute('maxlength');
    const emoji = '📈'.repeat(COMMENT_MAX_CHARS);
    fireEvent.change(box, { target: { value: emoji } });
    expect(box).toHaveValue(emoji);
    expect(screen.queryByText(/shorten the comment to post/)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Post comment on Recession' })).toBeEnabled();
  });

  it('flags an over-long comment and blocks posting', () => {
    renderComments();
    const box = screen.getByRole('textbox', { name: 'Add a comment on Recession' });
    fireEvent.change(box, { target: { value: 'c'.repeat(COMMENT_MAX_CHARS + 1) } });
    expect(screen.getByText(/shorten the comment to post/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Post comment on Recession' })).toBeDisabled();
  });
});
