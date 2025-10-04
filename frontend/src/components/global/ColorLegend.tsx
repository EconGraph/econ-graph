/**
 * ColorLegend Component.
 *
 * Color scale legend for the interactive world map showing value ranges
 * and color mapping for economic indicators.
 */

import React from 'react';
import { Box, Paper, Typography, Tooltip, Chip, Divider } from '@mui/material';
import { Info } from '@mui/icons-material';
import { ARIA_LABELS, getColorLegendDescription } from './accessibility-utils';

interface ColorLegendProps {
  /** Currently selected indicator. */
  indicator: string;
  /** Minimum value in the data range. */
  minValue: number;
  /** Maximum value in the data range. */
  maxValue: number;
  /** Color scheme being used. */
  colorScheme: string;
  /** Unit of measurement. */
  unit: string;
  /** Number of countries with data. */
  countriesWithData: number;
  /** Number of countries without data. */
  countriesWithoutData: number;
  /** Whether legend is visible. */
  visible?: boolean;
  /** Callback for color scheme change. */
  _onColorSchemeChange?: (scheme: string) => void;
}

const ColorLegend: React.FC<ColorLegendProps> = ({
  indicator,
  minValue,
  maxValue,
  colorScheme,
  unit,
  countriesWithData,
  countriesWithoutData,
  visible = true,
  _onColorSchemeChange,
}) => {
  if (!visible) return null;

  const formatValue = (value: number) => {
    if (value >= 1e12) {
      return `${(value / 1e12).toFixed(1)}T`;
    } else if (value >= 1e9) {
      return `${(value / 1e9).toFixed(1)}B`;
    } else if (value >= 1e6) {
      return `${(value / 1e6).toFixed(1)}M`;
    } else if (value >= 1e3) {
      return `${(value / 1e3).toFixed(1)}K`;
    } else {
      return value.toFixed(1);
    }
  };

  const getColorSchemeGradient = (scheme: string) => {
    switch (scheme) {
      case 'viridis':
        return 'linear-gradient(90deg, #440154, #482777, #3f4a8a, #31678e, #26838f, #1f9d8a, #6cce5a, #b6de2b, #fee825)';
      case 'blues':
        return 'linear-gradient(90deg, #f7fbff, #deebf7, #c6dbef, #9ecae1, #6baed6, #4292c6, #2171b5, #08519c, #08306b)';
      case 'reds':
        return 'linear-gradient(90deg, #fff5f0, #fee0d2, #fcbba1, #fc9272, #fb6a4a, #ef3b2c, #cb181d, #a50f15, #67000d)';
      case 'greens':
        return 'linear-gradient(90deg, #f7fcf5, #e5f5e0, #c7e9c0, #a1d99b, #74c476, #41ab5d, #238b45, #006d2c, #00441b)';
      default:
        return 'linear-gradient(90deg, #440154, #482777, #3f4a8a, #31678e, #26838f, #1f9d8a, #6cce5a, #b6de2b, #fee825)';
    }
  };

  const getColorSchemeLabel = (scheme: string) => {
    switch (scheme) {
      case 'viridis':
        return 'Viridis';
      case 'blues':
        return 'Blues';
      case 'reds':
        return 'Reds';
      case 'greens':
        return 'Greens';
      default:
        return scheme;
    }
  };

  const legendDescription = getColorLegendDescription(indicator, minValue, maxValue, unit);

  return (
    <Paper
      elevation={3}
      sx={{
        p: 2,
        minWidth: 280,
        maxWidth: 320,
        bgcolor: 'background.paper',
        borderRadius: 2,
      }}
      role='region'
      aria-label={ARIA_LABELS.COLOR_LEGEND}
      aria-describedby='legend-description'
    >
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 2 }}>
        <Typography variant='h6' id='legend-heading'>
          Color Legend
        </Typography>
        <Tooltip title='Color scale shows data distribution across countries'>
          <Info fontSize='small' color='action' />
        </Tooltip>
      </Box>

      <Typography
        id='legend-description'
        variant='caption'
        color='text.secondary'
        sx={{ mb: 2, display: 'block' }}
        aria-live='polite'
      >
        {legendDescription}
      </Typography>

      {/* Indicator Information */}
      <Box sx={{ mb: 2 }}>
        <Typography variant='subtitle1' fontWeight='bold'>
          {indicator}
        </Typography>
        <Typography variant='body2' color='text.secondary'>
          Unit: {unit}
        </Typography>
      </Box>

      {/* Color Scale */}
      <Box sx={{ mb: 2 }}>
        <Typography variant='subtitle2' gutterBottom id='color-scale-label'>
          Color Scale
        </Typography>
        <Box
          sx={{
            height: 20,
            background: getColorSchemeGradient(colorScheme),
            borderRadius: 1,
            border: '1px solid #e0e0e0',
            position: 'relative',
          }}
          role='img'
          aria-label={`Color scale from ${formatValue(minValue)} to ${formatValue(maxValue)}`}
          aria-describedby='color-scale-description'
        >
          {/* Value labels */}
          <Box
            sx={{
              position: 'absolute',
              top: -25,
              left: 0,
              right: 0,
              display: 'flex',
              justifyContent: 'space-between',
              fontSize: '0.75rem',
              color: 'text.secondary',
            }}
          >
            <span aria-label={`Minimum value: ${formatValue(minValue)}`}>
              {formatValue(minValue)}
            </span>
            <span aria-label={`Maximum value: ${formatValue(maxValue)}`}>
              {formatValue(maxValue)}
            </span>
          </Box>
        </Box>
        <Typography
          id='color-scale-description'
          variant='caption'
          color='text.secondary'
          sx={{ mt: 0.5, display: 'block' }}
        >
          Darker colors represent higher values, lighter colors represent lower values
        </Typography>
      </Box>

      {/* Data Coverage */}
      <Box sx={{ mb: 2 }}>
        <Typography variant='subtitle2' gutterBottom id='data-coverage-label'>
          Data Coverage
        </Typography>
        <Box
          sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}
          role='group'
          aria-labelledby='data-coverage-label'
        >
          <Chip
            label={`${countriesWithData} with data`}
            color='success'
            size='small'
            variant='outlined'
            aria-label={`${countriesWithData} countries have data available`}
          />
          {countriesWithoutData > 0 && (
            <Chip
              label={`${countriesWithoutData} no data`}
              color='default'
              size='small'
              variant='outlined'
              aria-label={`${countriesWithoutData} countries have no data available`}
            />
          )}
        </Box>
        <Typography variant='caption' color='text.secondary' sx={{ mt: 0.5, display: 'block' }}>
          {countriesWithData} out of {countriesWithData + countriesWithoutData} countries have data
        </Typography>
      </Box>

      <Divider sx={{ my: 2 }} />

      {/* Color Scheme Info */}
      <Box>
        <Typography variant='subtitle2' gutterBottom id='color-scheme-label'>
          Color Scheme
        </Typography>
        <Chip
          label={getColorSchemeLabel(colorScheme)}
          color='primary'
          size='small'
          variant='filled'
          aria-label={`Current color scheme: ${getColorSchemeLabel(colorScheme)}`}
        />
        <Typography variant='caption' display='block' sx={{ mt: 1 }} aria-live='polite'>
          {colorScheme === 'viridis' && 'Colorblind-friendly, good for most data'}
          {colorScheme === 'blues' && 'Professional appearance, good for positive indicators'}
          {colorScheme === 'reds' && 'Attention-grabbing, good for negative indicators'}
          {colorScheme === 'greens' && 'Natural appearance, good for environmental data'}
        </Typography>
      </Box>
    </Paper>
  );
};

export default ColorLegend;
