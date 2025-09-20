/**
 * MapLegend Component
 *
 * Legend component for displaying color scale and value ranges
 * for the world map visualization.
 */

import React from 'react';
import { Box, Typography, Paper } from '@mui/material';
import * as d3 from 'd3';
import { EconomicIndicator } from '../../types/globalAnalysis';

interface MapLegendProps {
  colorScale: d3.ScaleSequential<string, never>;
  indicator: EconomicIndicator;
  dataRange: { min: number; max: number };
}

const MapLegend: React.FC<MapLegendProps> = ({ colorScale, indicator, dataRange }) => {
  const numSegments = 100;
  const segmentWidth = 100 / numSegments;

  const gradientStops = d3.range(numSegments).map(i => {
    const value = dataRange.min + (i / numSegments) * (dataRange.max - dataRange.min);
    return { offset: `${i * segmentWidth}%`, color: colorScale(value) };
  });

  return (
    <Paper sx={{ p: 2, mt: 2, width: '100%', maxWidth: 300 }}>
      <Typography variant='subtitle2' gutterBottom>
        {indicator}
      </Typography>
      <Box
        sx={{
          width: '100%',
          height: 20,
          background: `linear-gradient(to right, ${gradientStops.map(s => `${s.color} ${s.offset}`).join(', ')})`,
          borderRadius: 1,
          mb: 1,
        }}
      />
      <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
        <Typography variant='caption'>{dataRange.min.toFixed(2)}</Typography>
        <Typography variant='caption'>{dataRange.max.toFixed(2)}</Typography>
      </Box>
      {/* Future: Add interactive slider for filtering or highlighting */}
    </Paper>
  );
};

export default MapLegend;
