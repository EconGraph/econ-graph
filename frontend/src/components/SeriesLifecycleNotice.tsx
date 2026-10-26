import React from 'react';
import { Alert, Box, Link, Typography } from '@mui/material';
import type { SeriesLifecycle } from '../utils/graphql';

/** Retained history stays usable; only a confirmed successor is described as a replacement. */
const SeriesLifecycleNotice: React.FC<{
  lifecycle?: SeriesLifecycle | null;
  sourceName?: string;
  compact?: boolean;
}> = ({ lifecycle, sourceName, compact = false }) => {
  if (!lifecycle || lifecycle.status === 'active' || lifecycle.status === 'restricted') {
    return null;
  }

  const since = lifecycle.since?.slice(0, 10);
  const message = `No longer published by ${sourceName ?? 'the source'}${since ? ` since ${since}` : ''}`;
  const successor = lifecycle.successorId;
  const proposed = !successor && lifecycle.proposedSuccessorId;

  const links = (
    <>
      {successor && (
        <Link
          href={`/series/${encodeURIComponent(successor)}`}
          onClick={event => event.stopPropagation()}
        >
          Superseded by another series
        </Link>
      )}
      {proposed && (
        <Link
          href={`/series/${encodeURIComponent(proposed)}`}
          onClick={event => event.stopPropagation()}
        >
          Possible successor (not confirmed)
        </Link>
      )}
    </>
  );

  return compact ? (
    <Box sx={{ mb: 2 }}>
      <Typography variant='body2' color='text.secondary'>
        {message}
      </Typography>
      {links}
    </Box>
  ) : (
    <Alert severity='info' sx={{ my: 2 }}>
      {message}. Stored history remains available.
      {(successor || proposed) && <Box sx={{ mt: 1 }}>{links}</Box>}
    </Alert>
  );
};

export default SeriesLifecycleNotice;
