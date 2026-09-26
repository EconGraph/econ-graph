import React from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { Box, Card, CardActionArea, CardContent, Chip, Skeleton, Typography } from '@mui/material';
import { Assessment as AssessmentIcon } from '@mui/icons-material';
import { format, isValid, parseISO } from 'date-fns';

import type { DashboardSeriesEntry } from '../../config/dashboardSeries';
import { useLatestObservation } from '../../hooks/useLatestObservation';
import { formatDecimalString } from '../../utils/formatDecimal';

/**
 * Format a `YYYY-MM-DD` date for display.
 * @param date - The ISO date.
 * @returns For example "Jul 1, 2024", or the input unchanged if it is not a date.
 */
function formatObservationDate(date: string): string {
  const parsed = parseISO(date);
  return isValid(parsed) ? format(parsed, 'MMM d, yyyy') : date;
}

interface LatestValueCardProps {
  entry: DashboardSeriesEntry;
}

/**
 * A dashboard card with a series' newest value and its observation date. It links to the
 * series page once the series is found, and says "No data yet" when the series is missing or
 * has no observations.
 * @param root0 - The component props.
 * @param root0.entry - The dashboard series to show.
 * @returns The card.
 */
export const LatestValueCard: React.FC<LatestValueCardProps> = ({ entry }) => {
  const {
    data: series,
    isLoading,
    isError,
  } = useLatestObservation(entry.sourceName, entry.externalId);
  const observation = series?.latestObservation ?? null;
  const headingId = `dashboard-card-${entry.id}`;

  let body: React.ReactNode;
  if (isLoading) {
    body = (
      <Skeleton variant='text' width={160} height={48} role='progressbar' aria-label='Loading' />
    );
  } else if (isError) {
    body = (
      <Typography variant='body1' color='text.secondary'>
        Couldn&apos;t load this series
      </Typography>
    );
  } else if (!series || !observation) {
    body = (
      <Typography variant='body1' color='text.secondary'>
        No data yet
      </Typography>
    );
  } else {
    const value =
      observation.value === null
        ? null
        : (formatDecimalString(observation.value, entry.fractionDigits) ?? observation.value);
    body = (
      <>
        <Typography variant='h4' component='div' sx={{ mb: 0.5, fontWeight: 600 }}>
          {value ?? 'No value reported'}
        </Typography>
        {series.units && (
          <Typography variant='body2' color='text.secondary'>
            {series.units}
          </Typography>
        )}
        <Typography variant='caption' color='text.secondary' component='div' sx={{ mt: 1 }}>
          <time dateTime={observation.date}>{formatObservationDate(observation.date)}</time>
        </Typography>
      </>
    );
  }

  const content = (
    <CardContent>
      <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
        <AssessmentIcon color='primary' sx={{ mr: 1, mt: 0.5 }} />
        <Box sx={{ flexGrow: 1 }}>
          <Typography
            id={headingId}
            variant='h6'
            component='h3'
            sx={{ fontSize: '1rem', lineHeight: 1.3 }}
          >
            {entry.label}
          </Typography>
          <Chip
            label={`${entry.sourceLabel} ${entry.externalId}`}
            size='small'
            variant='outlined'
            sx={{ mt: 0.5 }}
          />
        </Box>
      </Box>
      {body}
    </CardContent>
  );

  return (
    <Card component='article' aria-labelledby={headingId} sx={{ height: '100%' }}>
      {series ? (
        <CardActionArea
          component={RouterLink}
          aria-labelledby={headingId}
          to={`/series/${encodeURIComponent(series.id)}`}
          sx={{ height: '100%' }}
        >
          {content}
        </CardActionArea>
      ) : (
        content
      )}
    </Card>
  );
};
