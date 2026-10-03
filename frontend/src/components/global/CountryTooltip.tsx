/**
 * CountryTooltip Component.
 *
 * The world map's tooltip for the country under the pointer: its name, and its value with unit
 * and the value's date, or that it has no data.
 */

import React from 'react';
import { Typography, Paper } from '@mui/material';
import type { MapCountryValue } from './hooks/useWorldMapData';
import { formatMapDate, formatMapValue } from './mapFormat';

interface CountryTooltipProps {
  /** Country name. */
  name: string;
  /** The country's value; undefined when it has none. */
  entry: MapCountryValue | undefined;
  /** Unit of the value. */
  unit: string | null;
  /** Series frequency, e.g. `Annual`. */
  frequency: string | null;
  /** Pointer position within the map, in pixels. */
  x: number;
  y: number;
  /** The map's size; the tooltip opens away from the nearer edges so it stays inside. */
  boxWidth: number;
  boxHeight: number;
}

/** Gap between the pointer and the tooltip, in pixels. */
const OFFSET = 12;

const CountryTooltip: React.FC<CountryTooltipProps> = ({
  name,
  entry,
  unit,
  frequency,
  x,
  y,
  boxWidth,
  boxHeight,
}) => (
  <Paper
    role='tooltip'
    data-testid='country-tooltip'
    sx={{
      position: 'absolute',
      ...(x > boxWidth / 2 ? { right: boxWidth - x + OFFSET } : { left: x + OFFSET }),
      ...(y > boxHeight / 2 ? { bottom: boxHeight - y + OFFSET } : { top: y + OFFSET }),
      p: 1,
      // No wider than the room on the side it opens to, so a narrow map doesn't clip it.
      maxWidth: Math.max(120, Math.min(260, x > boxWidth / 2 ? x - OFFSET : boxWidth - x - OFFSET)),
      bgcolor: 'rgba(255, 255, 255, 0.95)',
      boxShadow: 3,
      pointerEvents: 'none',
      zIndex: 1000,
    }}
  >
    <Typography variant='subtitle2' sx={{ fontWeight: 'bold' }}>
      {name || 'Unknown'}
    </Typography>
    {entry ? (
      <>
        <Typography variant='body2'>
          {formatMapValue(entry.value)}
          {unit ? ` ${unit}` : ''}
        </Typography>
        <Typography variant='caption' color='text.secondary'>
          {formatMapDate(entry.date, frequency)}
        </Typography>
      </>
    ) : (
      <Typography variant='body2' color='text.secondary'>
        No data
      </Typography>
    )}
  </Paper>
);

export default CountryTooltip;
