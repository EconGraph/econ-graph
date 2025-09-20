/**
 * GlobalAnalysisDemo Page
 *
 * Demo page showcasing the interactive world map component
 * with all its features and controls.
 */

import React from 'react';
import * as d3 from 'd3';
import {
  Box,
  Container,
  Typography,
  Grid,
  Paper,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Chip,
  Alert,
} from '@mui/material';
import { GlobalAnalysisProvider, useGlobalAnalysis } from '../contexts/GlobalAnalysisContext';
import InteractiveWorldMap from '../components/global/InteractiveWorldMap';
import WorldMapControls from '../components/global/WorldMapControls';
// Note: CountryTooltip is imported for future use but not currently used in this component
import MapLegend from '../components/global/MapLegend';
import { sampleCountryData } from '../data/sampleCountryData';
import { CountryData, EconomicIndicator } from '../types/globalAnalysis';

const GlobalAnalysisDemoContent: React.FC = () => {
  const { state, actions } = useGlobalAnalysis();
  const { selectedCountries, selectedIndicator } = state;
  const { setSelectedIndicator } = actions;

  const availableIndicators: EconomicIndicator[] = ['gdp', 'inflation', 'unemployment'];

  const countriesWithData = React.useMemo(() => {
    return sampleCountryData.filter(
      country => country[selectedIndicator as keyof CountryData] !== undefined
    );
  }, [selectedIndicator]);

  const dataRange = React.useMemo(() => {
    const values = countriesWithData.map(
      country => country[selectedIndicator as keyof CountryData] as number
    );
    if (values.length === 0) return { min: 0, max: 1 };
    return {
      min: Math.min(...values),
      max: Math.max(...values),
    };
  }, [countriesWithData, selectedIndicator]);

  // Create color scale
  const colorScale = React.useMemo(() => {
    return d3.scaleSequential(d3.interpolateViridis).domain([dataRange.min, dataRange.max]);
  }, [dataRange]);

  return (
    <Container maxWidth='xl' sx={{ py: 4 }}>
      <Typography variant='h3' component='h1' gutterBottom>
        Global Analysis Demo
      </Typography>
      <Typography variant='h6' color='text.secondary' paragraph>
        Interactive World Map with Economic Data Visualization
      </Typography>

      <Grid container spacing={3}>
        <Grid item xs={12} md={9}>
          <Paper elevation={3} sx={{ p: 2, height: 550, display: 'flex', flexDirection: 'column' }}>
            <Box
              sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}
            >
              <FormControl size='small' sx={{ minWidth: 180 }}>
                <InputLabel id='indicator-select-label'>Economic Indicator</InputLabel>
                <Select
                  labelId='indicator-select-label'
                  value={selectedIndicator}
                  label='Economic Indicator'
                  onChange={e => setSelectedIndicator(e.target.value)}
                >
                  {availableIndicators.map(indicator => (
                    <MenuItem key={indicator} value={indicator}>
                      {indicator}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>
              <MapLegend
                colorScale={colorScale}
                indicator={selectedIndicator as EconomicIndicator}
                dataRange={dataRange}
              />
            </Box>
            <Box sx={{ flexGrow: 1, position: 'relative' }}>
              <InteractiveWorldMap
                data={sampleCountryData}
                selectedIndicator={selectedIndicator}
                timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
                onCountryClick={country => actions.toggleCountry(country.id)}
                onCountryHover={country => actions.setHoveredCountry(country?.id || null)}
                mapView={state.mapView}
                onMapViewChange={view => actions.setMapView(view)}
                width={900}
                height={500}
                projection={state.projection}
                colorScheme={state.colorScheme}
              />
            </Box>
          </Paper>
        </Grid>
        <Grid item xs={12} md={3}>
          <WorldMapControls />
          <Paper elevation={3} sx={{ p: 2, mt: 3 }}>
            <Typography variant='h6' gutterBottom>
              Selected Countries
            </Typography>
            {selectedCountries.length === 0 ? (
              <Alert severity='info'>Click on countries to select them.</Alert>
            ) : (
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>
                {selectedCountries.map(countryId => (
                  <Chip
                    key={countryId}
                    label={countryId}
                    onDelete={() => actions.toggleCountry(countryId)}
                    color='primary'
                    variant='outlined'
                  />
                ))}
              </Box>
            )}
          </Paper>
        </Grid>
      </Grid>
    </Container>
  );
};

const GlobalAnalysisDemo: React.FC = () => {
  return (
    <GlobalAnalysisProvider>
      <GlobalAnalysisDemoContent />
    </GlobalAnalysisProvider>
  );
};

export default GlobalAnalysisDemo;
