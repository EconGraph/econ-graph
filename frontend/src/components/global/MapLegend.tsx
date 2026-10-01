/**
 * MapLegend Component.
 *
 * The world map's legend: the indicator and its unit, the color scale with the value range, the
 * fill of countries without data, and the dates the values come from. Each country shows its
 * own latest value, so the dates can differ between countries; the legend says so.
 */

import React from 'react';
import { Box, Typography, Paper } from '@mui/material';
import * as d3 from 'd3';
import { formatMapDate, NO_DATA_FILL } from './mapFormat';

interface MapLegendProps {
  colorScale: (value: number) => string;
  /** Indicator name. */
  indicator: string;
  unit: string | null;
  frequency: string | null;
  dataRange: { min: number; max: number };
  /** Earliest and latest value dates; null when no country has a value. */
  dateRange: { earliest: string; latest: string } | null;
  /** Number of countries with a value on the map. */
  countryCount: number;
  /** Number of countries with a value but no shape on the map, such as Kosovo. */
  undrawnCount: number;
}

const formatBound = (value: number) => value.toLocaleString('en-US', { maximumFractionDigits: 2 });

const MapLegend: React.FC<MapLegendProps> = ({
  colorScale,
  indicator,
  unit,
  frequency,
  dataRange,
  dateRange,
  countryCount,
  undrawnCount,
}) => {
  const numSegments = 20;
  const gradientStops = d3.range(numSegments + 1).map(i => {
    const value = dataRange.min + (i / numSegments) * (dataRange.max - dataRange.min);
    return `${colorScale(value)} ${(i / numSegments) * 100}%`;
  });

  let datesNote: string | null = null;
  if (dateRange) {
    const earliest = formatMapDate(dateRange.earliest, frequency);
    const latest = formatMapDate(dateRange.latest, frequency);
    datesNote =
      earliest === latest
        ? `Latest values, all for ${latest}.`
        : `Each country's latest value, so dates differ: ${earliest} to ${latest}.`;
  }

  return (
    <Paper variant='outlined' sx={{ p: 2 }} data-testid='map-legend'>
      <Typography variant='subtitle2'>{indicator}</Typography>
      {unit && (
        <Typography variant='caption' color='text.secondary' component='div'>
          Unit: {unit}
        </Typography>
      )}
      {countryCount > 0 ? (
        <>
          <Box
            sx={{
              mt: 1,
              height: 16,
              background: `linear-gradient(to right, ${gradientStops.join(', ')})`,
              borderRadius: 1,
            }}
          />
          <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
            <Typography variant='caption'>{formatBound(dataRange.min)}</Typography>
            <Typography variant='caption'>{formatBound(dataRange.max)}</Typography>
          </Box>
        </>
      ) : null}
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mt: 1 }}>
        <Box
          sx={{ width: 16, height: 12, bgcolor: NO_DATA_FILL, borderRadius: 0.5, flexShrink: 0 }}
        />
        <Typography variant='caption'>No data</Typography>
      </Box>
      <Typography variant='caption' color='text.secondary' component='div' sx={{ mt: 1 }}>
        {countryCount === 1 ? '1 country' : `${countryCount} countries`} with data.
        {datesNote ? ` ${datesNote}` : ''}
        {undrawnCount > 0
          ? ` ${undrawnCount === 1 ? '1 more has' : `${undrawnCount} more have`} no shape on the map; see the table.`
          : ''}
      </Typography>
    </Paper>
  );
};

export default MapLegend;
