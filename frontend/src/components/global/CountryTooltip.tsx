/**
 * CountryTooltip Component.
 *
 * Tooltip component for displaying country-specific data when hovering
 * over countries on the world map.
 */

import React from 'react';
import { Box, Typography, Paper } from '@mui/material';
import { CountryData, EconomicIndicator } from '../../types/globalAnalysis';
import * as d3 from 'd3';

interface CountryTooltipProps {
  countryId: string;
  countryData: CountryData | undefined;
  indicator: EconomicIndicator;
  colorScale: d3.ScaleSequential<string, never>;
}

const CountryTooltip: React.FC<CountryTooltipProps> = ({
  countryId,
  countryData,
  indicator,
  colorScale,
}) => {
  if (!countryData) return null;

  const indicatorValue = countryData[indicator as keyof CountryData] as number;
  const formattedValue = typeof indicatorValue === 'number' ? indicatorValue.toFixed(2) : 'N/A';
  const indicatorColor = typeof indicatorValue === 'number' ? colorScale(indicatorValue) : '#ccc';

  return (
    <Paper
      sx={{
        position: 'absolute',
        top: 0, // Will be positioned dynamically by D3 in the future
        left: 0, // Will be positioned dynamically by D3 in the future
        p: 1,
        bgcolor: 'rgba(255, 255, 255, 0.9)',
        boxShadow: 3,
        pointerEvents: 'none',
        zIndex: 1000,
      }}
    >
      <Typography variant='subtitle2' sx={{ fontWeight: 'bold' }}>
        {countryData.name} ({countryId})
      </Typography>
      <Typography variant='body2'>
        {indicator}:{' '}
        <Box component='span' sx={{ color: indicatorColor, fontWeight: 'bold' }}>
          {formattedValue}
        </Box>
      </Typography>
      {/* Add more details as needed */}
    </Paper>
  );
};

export default CountryTooltip;
