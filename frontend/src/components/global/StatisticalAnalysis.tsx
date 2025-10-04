/**
 * StatisticalAnalysis Component.
 *
 * Statistical analysis panel showing descriptive statistics, correlations,
 * and data insights for selected countries and indicators.
 */

import React from 'react';
import {
  Box,
  Paper,
  Typography,
  Grid,
  Card,
  CardContent,
  Chip,
  Divider,
  LinearProgress,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Button,
  Accordion,
  AccordionSummary,
  AccordionDetails,
} from '@mui/material';
import {
  TrendingUp,
  TrendingDown,
  BarChart,
  ScatterPlot,
  ExpandMore,
  Download,
} from '@mui/icons-material';
import { CountryData } from '../../types/globalAnalysis';

interface StatisticalAnalysisProps {
  /** Array of selected countries */
  selectedCountries: CountryData[];
  /** Currently selected indicator */
  selectedIndicator: string;
  /** Whether analysis is visible */
  visible?: boolean;
  /** Callback when data is exported */
  onExportData?: () => void;
}

interface Statistics {
  count: number;
  mean: number;
  median: number;
  min: number;
  max: number;
  stdDev: number;
  q25: number;
  q75: number;
  range: number;
}

const StatisticalAnalysis: React.FC<StatisticalAnalysisProps> = ({
  selectedCountries,
  selectedIndicator,
  visible = true,
  onExportData,
}) => {
  const [expanded, setExpanded] = React.useState<string | false>('descriptive');

  if (!visible) return null;

  const getCountryValue = (country: CountryData) => {
    const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
    return indicator?.value || 0;
  };

  const getCountryUnit = (country: CountryData) => {
    const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
    return indicator?.unit || '';
  };

  const calculateStatistics = (): Statistics => {
    const values = selectedCountries
      .map(getCountryValue)
      .filter(value => value > 0)
      .sort((a, b) => a - b);

    if (values.length === 0) {
      return {
        count: 0,
        mean: 0,
        median: 0,
        min: 0,
        max: 0,
        stdDev: 0,
        q25: 0,
        q75: 0,
        range: 0,
      };
    }

    const count = values.length;
    const mean = values.reduce((sum, val) => sum + val, 0) / count;
    const median = values[Math.floor(count / 2)];
    const min = Math.min(...values);
    const max = Math.max(...values);
    const range = max - min;

    // Standard deviation
    const variance = values.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / count;
    const stdDev = Math.sqrt(variance);

    // Quartiles
    const q25 = values[Math.floor(count * 0.25)];
    const q75 = values[Math.floor(count * 0.75)];

    return {
      count,
      mean,
      median,
      min,
      max,
      stdDev,
      q25,
      q75,
      range,
    };
  };

  const formatValue = (value: number, unit: string) => {
    if (value >= 1e12) {
      return `${(value / 1e12).toFixed(1)}T ${unit}`;
    } else if (value >= 1e9) {
      return `${(value / 1e9).toFixed(1)}B ${unit}`;
    } else if (value >= 1e6) {
      return `${(value / 1e6).toFixed(1)}M ${unit}`;
    } else if (value >= 1e3) {
      return `${(value / 1e3).toFixed(1)}K ${unit}`;
    } else {
      return `${value.toFixed(1)} ${unit}`;
    }
  };

  const getTopCountries = () => {
    return selectedCountries
      .filter(country => getCountryValue(country) > 0)
      .sort((a, b) => getCountryValue(b) - getCountryValue(a))
      .slice(0, 5);
  };

  const getBottomCountries = () => {
    return selectedCountries
      .filter(country => getCountryValue(country) > 0)
      .sort((a, b) => getCountryValue(a) - getCountryValue(b))
      .slice(0, 5);
  };

  const getRegionDistribution = () => {
    const regions = selectedCountries.reduce(
      (acc, country) => {
        const key = country.region ?? 'Unknown';
        acc[key] = (acc[key] || 0) + 1;
        return acc;
      },
      {} as Record<string, number>
    );

    return Object.entries(regions).map(([region, count]) => ({
      region,
      count,
      percentage: (count / selectedCountries.length) * 100,
    }));
  };

  const statistics = calculateStatistics();
  const topCountries = getTopCountries();
  const _bottomCountries = getBottomCountries();
  const regionDistribution = getRegionDistribution();
  const unit = selectedCountries[0] ? getCountryUnit(selectedCountries[0]) : '';

  const handleAccordionChange =
    (panel: string) => (event: React.SyntheticEvent, isExpanded: boolean) => {
      setExpanded(isExpanded ? panel : false);
    };

  return (
    <Paper
      elevation={3}
      sx={{
        p: 2,
        minWidth: 400,
        maxWidth: 600,
        bgcolor: 'background.paper',
        borderRadius: 2,
        maxHeight: '80vh',
        overflow: 'auto',
      }}
    >
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
        <Typography variant='h6'>Statistical Analysis</Typography>
        {onExportData && (
          <Button variant='outlined' size='small' startIcon={<Download />} onClick={onExportData}>
            Export
          </Button>
        )}
      </Box>

      {/* Descriptive Statistics */}
      <Accordion
        expanded={expanded === 'descriptive'}
        onChange={handleAccordionChange('descriptive')}
      >
        <AccordionSummary expandIcon={<ExpandMore />}>
          <Typography variant='subtitle1'>Descriptive Statistics</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Grid container spacing={2}>
            <Grid item xs={6}>
              <Card variant='outlined'>
                <CardContent>
                  <Typography variant='h6' color='primary'>
                    {statistics.count}
                  </Typography>
                  <Typography variant='body2' color='text.secondary'>
                    Countries with Data
                  </Typography>
                </CardContent>
              </Card>
            </Grid>
            <Grid item xs={6}>
              <Card variant='outlined'>
                <CardContent>
                  <Typography variant='h6' color='primary'>
                    {formatValue(statistics.mean, unit)}
                  </Typography>
                  <Typography variant='body2' color='text.secondary'>
                    Average
                  </Typography>
                </CardContent>
              </Card>
            </Grid>
            <Grid item xs={6}>
              <Card variant='outlined'>
                <CardContent>
                  <Typography variant='h6' color='primary'>
                    {formatValue(statistics.median, unit)}
                  </Typography>
                  <Typography variant='body2' color='text.secondary'>
                    Median
                  </Typography>
                </CardContent>
              </Card>
            </Grid>
            <Grid item xs={6}>
              <Card variant='outlined'>
                <CardContent>
                  <Typography variant='h6' color='primary'>
                    {formatValue(statistics.stdDev, unit)}
                  </Typography>
                  <Typography variant='body2' color='text.secondary'>
                    Std Deviation
                  </Typography>
                </CardContent>
              </Card>
            </Grid>
          </Grid>
        </AccordionDetails>
      </Accordion>

      {/* Value Range */}
      <Accordion expanded={expanded === 'range'} onChange={handleAccordionChange('range')}>
        <AccordionSummary expandIcon={<ExpandMore />}>
          <Typography variant='subtitle1'>Value Range</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Box sx={{ mb: 2 }}>
            <Typography variant='body2' gutterBottom>
              Range: {formatValue(statistics.min, unit)} - {formatValue(statistics.max, unit)}
            </Typography>
            <LinearProgress variant='determinate' value={100} sx={{ height: 8, borderRadius: 4 }} />
          </Box>

          <Grid container spacing={2}>
            <Grid item xs={6}>
              <Typography variant='body2' color='text.secondary'>
                Min: {formatValue(statistics.min, unit)}
              </Typography>
            </Grid>
            <Grid item xs={6}>
              <Typography variant='body2' color='text.secondary'>
                Max: {formatValue(statistics.max, unit)}
              </Typography>
            </Grid>
            <Grid item xs={6}>
              <Typography variant='body2' color='text.secondary'>
                Q25: {formatValue(statistics.q25, unit)}
              </Typography>
            </Grid>
            <Grid item xs={6}>
              <Typography variant='body2' color='text.secondary'>
                Q75: {formatValue(statistics.q75, unit)}
              </Typography>
            </Grid>
          </Grid>
        </AccordionDetails>
      </Accordion>

      {/* Top Countries */}
      <Accordion expanded={expanded === 'top'} onChange={handleAccordionChange('top')}>
        <AccordionSummary expandIcon={<ExpandMore />}>
          <Typography variant='subtitle1'>Top Countries</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <TableContainer>
            <Table size='small'>
              <TableHead>
                <TableRow>
                  <TableCell>Country</TableCell>
                  <TableCell align='right'>Value</TableCell>
                  <TableCell align='right'>Rank</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {topCountries.map((country, index) => (
                  <TableRow key={country.id}>
                    <TableCell>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        <Typography variant='body2'>{country.name}</Typography>
                        <Chip label={country.isoAlpha2} size='small' />
                      </Box>
                    </TableCell>
                    <TableCell align='right'>
                      {formatValue(getCountryValue(country), getCountryUnit(country))}
                    </TableCell>
                    <TableCell align='right'>
                      <Chip label={index + 1} size='small' color='primary' />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </AccordionDetails>
      </Accordion>

      {/* Regional Distribution */}
      <Accordion expanded={expanded === 'regions'} onChange={handleAccordionChange('regions')}>
        <AccordionSummary expandIcon={<ExpandMore />}>
          <Typography variant='subtitle1'>Regional Distribution</Typography>
        </AccordionSummary>
        <AccordionDetails>
          {regionDistribution.map(({ region, count, percentage }) => (
            <Box key={region} sx={{ mb: 2 }}>
              <Box sx={{ display: 'flex', justifyContent: 'space-between', mb: 1 }}>
                <Typography variant='body2'>{region}</Typography>
                <Typography variant='body2'>
                  {count} countries ({percentage.toFixed(1)}%)
                </Typography>
              </Box>
              <LinearProgress
                variant='determinate'
                value={percentage}
                sx={{ height: 6, borderRadius: 3 }}
              />
            </Box>
          ))}
        </AccordionDetails>
      </Accordion>

      {/* Data Quality */}
      <Accordion expanded={expanded === 'quality'} onChange={handleAccordionChange('quality')}>
        <AccordionSummary expandIcon={<ExpandMore />}>
          <Typography variant='subtitle1'>Data Quality</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
            <Chip label={`${statistics.count} with data`} color='success' variant='outlined' />
            <Chip
              label={`${selectedCountries.length - statistics.count} missing`}
              color='default'
              variant='outlined'
            />
            <Chip
              label={`${((statistics.count / selectedCountries.length) * 100).toFixed(1)}% coverage`}
              color='primary'
              variant='outlined'
            />
          </Box>
        </AccordionDetails>
      </Accordion>
    </Paper>
  );
};

export default StatisticalAnalysis;
