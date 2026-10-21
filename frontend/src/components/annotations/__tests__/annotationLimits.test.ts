/**
 * REQUIREMENT: Annotation and comment length limits match the backend's, and long titles are
 * cut on the chart.
 * PURPOSE: Regression tests for ECO-402. The dialog's limits only help if the backend refuses
 * the same lengths, so this reads the backend's constants and fails when the two drift apart.
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import {
  ANNOTATION_NOTE_MAX_CHARS,
  ANNOTATION_TITLE_MAX_CHARS,
  CHART_LABEL_MAX_CHARS,
  COMMENT_MAX_CHARS,
  charCount,
  truncateLabel,
} from '../annotationLimits';

const backendSource = readFileSync(
  resolve(
    import.meta.dirname,
    '../../../../../backend/crates/econ-graph-services/src/services/collaboration_service.rs'
  ),
  'utf8'
);

/** The value of `pub const <name>: usize = <value>;` in the backend source. */
const backendLimit = (name: string): number => {
  const match = backendSource.match(new RegExp(`pub const ${name}: usize = ([\\d_]+);`));
  if (!match) throw new Error(`${name} not found in collaboration_service.rs`);
  return Number(match[1].replace(/_/g, ''));
};

describe('annotation limits', () => {
  it("match the backend's", () => {
    expect(backendLimit('ANNOTATION_TITLE_MAX_CHARS')).toBe(ANNOTATION_TITLE_MAX_CHARS);
    expect(backendLimit('ANNOTATION_NOTE_MAX_CHARS')).toBe(ANNOTATION_NOTE_MAX_CHARS);
    expect(backendLimit('COMMENT_MAX_CHARS')).toBe(COMMENT_MAX_CHARS);
  });
});

describe('truncateLabel', () => {
  it('keeps labels up to the limit as they are', () => {
    const label = 'x'.repeat(CHART_LABEL_MAX_CHARS);
    expect(truncateLabel(label)).toBe(label);
  });

  it('cuts longer labels to the limit, ellipsis included', () => {
    const cut = truncateLabel('y'.repeat(CHART_LABEL_MAX_CHARS + 1));
    expect(Array.from(cut)).toHaveLength(CHART_LABEL_MAX_CHARS);
    expect(cut.endsWith('…')).toBe(true);
  });

  it('counts characters, not UTF-16 code units', () => {
    expect(truncateLabel('😀😀😀', 3)).toBe('😀😀😀');
    expect(truncateLabel('😀😀😀😀', 3)).toBe('😀😀…');
  });
});

describe('charCount', () => {
  it('counts an emoji once, as the backend does', () => {
    expect(charCount('GDP 📈')).toBe(5);
  });
});
