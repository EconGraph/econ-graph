import React from 'react';
import { Box, Link, Typography } from '@mui/material';
import type { SourceNotice } from '../../hooks/useSeriesData';
import { derivedValuesNote, retrievalDate } from '../../utils/sourceNotice';

export interface SourceNoticePanelProps {
  /** The source's name; empty when unknown. */
  sourceName: string;
  /** The source's required notice; null or absent for a source without one. */
  notice?: SourceNotice | null;
  /** When EconGraph last retrieved the series from its source; null when unknown. */
  lastUpdated: string | null;
  /** The shown transformation's label, such as "Year-over-Year % Change"; empty for levels. */
  transformationLabel: string;
  /** Distinguish repeated source-notice landmarks, such as dashboard cards. */
  ariaLabel?: string;
}

/**
 * The attribution shown with a series: the citation and notice its source's terms require,
 * the retrieval date where the terms ask for it, and a statement when the shown values are
 * EconGraph's transformation rather than the source's figures. Renders nothing when there is
 * nothing to say.
 * @param props - See `SourceNoticePanelProps`.
 * @returns The notice block, or null.
 */
export function SourceNoticePanel({
  sourceName,
  notice,
  lastUpdated,
  transformationLabel,
  ariaLabel = 'Source notice',
}: SourceNoticePanelProps): React.ReactElement | null {
  const derived = derivedValuesNote(transformationLabel, sourceName);
  if (!notice && !derived) return null;
  const retrieved = notice?.retrievalDateRequired ? retrievalDate(lastUpdated) : null;
  return (
    <Box
      component='section'
      aria-label={ariaLabel}
      data-testid='source-notice'
      sx={{ mt: 2, color: 'text.secondary' }}
    >
      {derived && (
        <Typography variant='body2' gutterBottom>
          {derived}
        </Typography>
      )}
      {notice?.citation && (
        <Typography variant='body2' gutterBottom>
          {notice.citation}
        </Typography>
      )}
      {notice && (
        <Typography variant='body2' gutterBottom>
          {notice.notice}
          {notice.retrievalDateRequired &&
            (retrieved
              ? ` Data retrieved from ${sourceName || 'the source'} on ${retrieved}.`
              : ' Retrieval date not recorded.')}{' '}
          <Link href={notice.termsUrl} target='_blank' rel='noopener noreferrer'>
            Terms of use
          </Link>
        </Typography>
      )}
    </Box>
  );
}

export default SourceNoticePanel;
