import React, { useState, useEffect, useRef } from 'react';
import {
  Box,
  Paper,
  Typography,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Slider,
  Grid,
  IconButton,
  Tooltip,
  CircularProgress,
  Chip,
  Autocomplete,
  TextField,
  ListSubheader,
} from '@mui/material';
import { Info, ZoomIn, ZoomOut, RestartAlt } from '@mui/icons-material';
import * as d3 from 'd3';
import * as topojson from 'topojson-client';

// Types for the component
interface CountryData {
  id: string;
  name: string;
  isoAlpha2: string;
  isoAlpha3: string;
  latitude: number;
  longitude: number;
  gdpUsd?: number;
  population?: number;
  region?: string;
  subregion?: string;
}

interface CorrelationData {
  countryAId: string;
  countryBId: string;
  correlationCoefficient: number;
  pValue?: number;
  countryA: { name: string; isoAlpha2: string };
  countryB: { name: string; isoAlpha2: string };
}

// Extended country data with regional grouping
const sampleCountries: CountryData[] = [
  {
    id: '1',
    name: 'United States',
    isoAlpha2: 'US',
    isoAlpha3: 'USA',
    latitude: 39.8283,
    longitude: -98.5795,
    gdpUsd: 23000000000000,
    population: 331000000,
    region: 'Americas',
    subregion: 'North America',
  },
  {
    id: '2',
    name: 'China',
    isoAlpha2: 'CN',
    isoAlpha3: 'CHN',
    latitude: 35.8617,
    longitude: 104.1954,
    gdpUsd: 17700000000000,
    population: 1441000000,
    region: 'Asia',
    subregion: 'East Asia',
  },
  {
    id: '3',
    name: 'Japan',
    isoAlpha2: 'JP',
    isoAlpha3: 'JPN',
    latitude: 36.2048,
    longitude: 138.2529,
    gdpUsd: 5000000000000,
    population: 126000000,
    region: 'Asia',
    subregion: 'East Asia',
  },
  {
    id: '4',
    name: 'Germany',
    isoAlpha2: 'DE',
    isoAlpha3: 'DEU',
    latitude: 51.1657,
    longitude: 10.4515,
    gdpUsd: 4200000000000,
    population: 83000000,
    region: 'Europe',
    subregion: 'Western Europe',
  },
  {
    id: '5',
    name: 'United Kingdom',
    isoAlpha2: 'GB',
    isoAlpha3: 'GBR',
    latitude: 55.3781,
    longitude: -3.436,
    gdpUsd: 3100000000000,
    population: 67000000,
    region: 'Europe',
    subregion: 'Northern Europe',
  },
  {
    id: '6',
    name: 'France',
    isoAlpha2: 'FR',
    isoAlpha3: 'FRA',
    latitude: 46.2276,
    longitude: 2.2137,
    gdpUsd: 2900000000000,
    population: 67000000,
    region: 'Europe',
    subregion: 'Western Europe',
  },
  {
    id: '7',
    name: 'India',
    isoAlpha2: 'IN',
    isoAlpha3: 'IND',
    latitude: 20.5937,
    longitude: 78.9629,
    gdpUsd: 3700000000000,
    population: 1380000000,
    region: 'Asia',
    subregion: 'South Asia',
  },
  {
    id: '8',
    name: 'Brazil',
    isoAlpha2: 'BR',
    isoAlpha3: 'BRA',
    latitude: -14.235,
    longitude: -51.9253,
    gdpUsd: 2100000000000,
    population: 215000000,
    region: 'Americas',
    subregion: 'South America',
  },
  {
    id: '9',
    name: 'Canada',
    isoAlpha2: 'CA',
    isoAlpha3: 'CAN',
    latitude: 56.1304,
    longitude: -106.3468,
    gdpUsd: 1900000000000,
    population: 38000000,
    region: 'Americas',
    subregion: 'North America',
  },
  {
    id: '10',
    name: 'Italy',
    isoAlpha2: 'IT',
    isoAlpha3: 'ITA',
    latitude: 41.8719,
    longitude: 12.5674,
    gdpUsd: 2100000000000,
    population: 60000000,
    region: 'Europe',
    subregion: 'Southern Europe',
  },
  {
    id: '11',
    name: 'South Korea',
    isoAlpha2: 'KR',
    isoAlpha3: 'KOR',
    latitude: 35.9078,
    longitude: 127.7669,
    gdpUsd: 1800000000000,
    population: 52000000,
    region: 'Asia',
    subregion: 'East Asia',
  },
  {
    id: '12',
    name: 'Australia',
    isoAlpha2: 'AU',
    isoAlpha3: 'AUS',
    latitude: -25.2744,
    longitude: 133.7751,
    gdpUsd: 1500000000000,
    population: 26000000,
    region: 'Oceania',
    subregion: 'Australia and New Zealand',
  },
];

const sampleCorrelations: CorrelationData[] = [
  {
    countryAId: '1',
    countryBId: '2',
    correlationCoefficient: 0.75,
    pValue: 0.001,
    countryA: { name: 'United States', isoAlpha2: 'US' },
    countryB: { name: 'China', isoAlpha2: 'CN' },
  },
  {
    countryAId: '1',
    countryBId: '3',
    correlationCoefficient: 0.68,
    pValue: 0.002,
    countryA: { name: 'United States', isoAlpha2: 'US' },
    countryB: { name: 'Japan', isoAlpha2: 'JP' },
  },
  {
    countryAId: '2',
    countryBId: '4',
    correlationCoefficient: 0.62,
    pValue: 0.005,
    countryA: { name: 'China', isoAlpha2: 'CN' },
    countryB: { name: 'Germany', isoAlpha2: 'DE' },
  },
  {
    countryAId: '1',
    countryBId: '9',
    correlationCoefficient: 0.82,
    pValue: 0.001,
    countryA: { name: 'United States', isoAlpha2: 'US' },
    countryB: { name: 'Canada', isoAlpha2: 'CA' },
  },
  {
    countryAId: '4',
    countryBId: '6',
    correlationCoefficient: 0.79,
    pValue: 0.001,
    countryA: { name: 'Germany', isoAlpha2: 'DE' },
    countryB: { name: 'France', isoAlpha2: 'FR' },
  },
  {
    countryAId: '3',
    countryBId: '11',
    correlationCoefficient: 0.71,
    pValue: 0.002,
    countryA: { name: 'Japan', isoAlpha2: 'JP' },
    countryB: { name: 'South Korea', isoAlpha2: 'KR' },
  },
  {
    countryAId: '2',
    countryBId: '7',
    correlationCoefficient: 0.58,
    pValue: 0.01,
    countryA: { name: 'China', isoAlpha2: 'CN' },
    countryB: { name: 'India', isoAlpha2: 'IN' },
  },
  {
    countryAId: '5',
    countryBId: '4',
    correlationCoefficient: 0.66,
    pValue: 0.003,
    countryA: { name: 'United Kingdom', isoAlpha2: 'GB' },
    countryB: { name: 'Germany', isoAlpha2: 'DE' },
  },
];

const GlobalEconomicNetworkMap: React.FC = () => {
  const svgRef = useRef<SVGSVGElement>(null);
  const [selectedIndicator, setSelectedIndicator] = useState<string>('gdp');
  const [minCorrelation, setMinCorrelation] = useState<number>(0.5);
  const [selectedCountry, setSelectedCountry] = useState<CountryData | null>(null);
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>(
    sampleCountries.slice(0, 5)
  ); // Start with first 5
  const [selectedRegion, setSelectedRegion] = useState<string>('all');
  const [loading] = useState<boolean>(false);

  // Get unique regions for filtering
  const regions = Array.from(new Set(sampleCountries.map(c => c.region))).sort();

  // Filter countries based on selected region
  const filteredCountries =
    selectedRegion === 'all'
      ? sampleCountries
      : sampleCountries.filter(c => c.region === selectedRegion);

  // Group countries by region for the dropdown (currently unused but kept for future features)
  // const countriesByRegion = sampleCountries.reduce((acc, country) => {
  //   const region = country.region || 'Other';
  //   if (!acc[region]) acc[region] = [];
  //   acc[region].push(country);
  //   return acc;
  // }, {} as Record<string, CountryData[]>);

  // Initialize the map
  useEffect(() => {
    if (!svgRef.current) return;

    const svg = d3.select(svgRef.current);
    svg.selectAll('*').remove();

    const width = 800;
    const height = 500;

    // Set up projection
    const projection = d3
      .geoNaturalEarth1()
      .scale(130)
      .translate([width / 2, height / 2]);

    const path = d3.geoPath().projection(projection);

    // Create main group
    const g = svg.append('g');

    // Add world map background
    g.append('rect')
      .attr('width', width)
      .attr('height', height)
      .attr('fill', '#f8f9fa')
      .attr('stroke', '#e0e0e0')
      .attr('stroke-width', '2');

    // Load and render world map
    d3.json('https://cdn.jsdelivr.net/npm/world-atlas@3/world/110m.json')
      .then((world: any) => {
        // Draw world countries
        const worldFeatures = topojson.feature(world, world.objects.countries) as any;
        g.selectAll('.world-country')
          .data(worldFeatures.features)
          .enter()
          .append('path')
          .attr('class', 'world-country')
          .attr('d', (d: any) => path(d))
          .attr('fill', '#e8e8e8')
          .attr('stroke', '#ffffff')
          .attr('stroke-width', '0.5')
          .style('opacity', '0.7');

        // Add economic data circles for selected countries
        g.selectAll('.economic-country')
          .data(selectedCountries)
          .enter()
          .append('circle')
          .attr('class', 'economic-country')
          .attr('cx', d => projection([d.longitude, d.latitude])?.[0] || 0)
          .attr('cy', d => projection([d.longitude, d.latitude])?.[1] || 0)
          .attr('r', d => Math.sqrt((d.gdpUsd || 0) / 1e12) * 8 + 5)
          .attr('fill', '#1976d2')
          .attr('stroke', '#0d47a1')
          .attr('stroke-width', 3)
          .style('opacity', '0.8')
          .style('cursor', 'pointer')
          .on('click', (event, d) => {
            setSelectedCountry(d);
          })
          .on('mouseover', function (event, d) {
            d3.select(this)
              .transition()
              .duration(200)
              .attr('r', Math.sqrt((d.gdpUsd || 0) / 1e12) * 8 + 8)
              .attr('fill', '#42a5f5')
              .style('opacity', '1')
              .style('filter', 'drop-shadow(0 4px 8px rgba(0,0,0,0.3))');
          })
          .on('mouseout', function (event, d) {
            d3.select(this)
              .transition()
              .duration(200)
              .attr('r', Math.sqrt((d.gdpUsd || 0) / 1e12) * 8 + 5)
              .attr('fill', '#1976d2')
              .style('opacity', '0.8')
              .style('filter', 'none');
          });

        // Add country labels
        g.selectAll('.country-label')
          .data(selectedCountries)
          .enter()
          .append('text')
          .attr('class', 'country-label')
          .attr('x', d => (projection([d.longitude, d.latitude])?.[0] || 0) + 15)
          .attr('y', d => (projection([d.longitude, d.latitude])?.[1] || 0) + 5)
          .text(d => d.name)
          .attr('font-size', '14px')
          .attr('font-weight', '600')
          .attr('fill', '#2c3e50')
          .style('pointer-events', 'none')
          .style('text-shadow', '1px 1px 2px rgba(255,255,255,0.8)');

        // Add correlation lines - only show correlations between selected countries
        const selectedCountryIds = selectedCountries.map(c => c.id);
        const filteredCorrelations = sampleCorrelations.filter(
          corr =>
            Math.abs(corr.correlationCoefficient) >= minCorrelation &&
            selectedCountryIds.includes(corr.countryAId) &&
            selectedCountryIds.includes(corr.countryBId)
        );

        g.selectAll('.correlation-line')
          .data(filteredCorrelations)
          .enter()
          .append('line')
          .attr('class', 'correlation-line')
          .attr('x1', d => {
            const country = selectedCountries.find(c => c.id === d.countryAId);
            return projection([country?.longitude || 0, country?.latitude || 0])?.[0] || 0;
          })
          .attr('y1', d => {
            const country = selectedCountries.find(c => c.id === d.countryAId);
            return projection([country?.longitude || 0, country?.latitude || 0])?.[1] || 0;
          })
          .attr('x2', d => {
            const country = selectedCountries.find(c => c.id === d.countryBId);
            return projection([country?.longitude || 0, country?.latitude || 0])?.[0] || 0;
          })
          .attr('y2', d => {
            const country = selectedCountries.find(c => c.id === d.countryBId);
            return projection([country?.longitude || 0, country?.latitude || 0])?.[1] || 0;
          })
          .attr('stroke', d => (d.correlationCoefficient > 0 ? '#1976d2' : '#d32f2f'))
          .attr('stroke-width', d => Math.abs(d.correlationCoefficient) * 6 + 2)
          .attr('opacity', 0.8)
          .style('stroke-dasharray', d => (d.correlationCoefficient > 0 ? 'none' : '5,5'));
      })
      .catch(error => {
        console.error('Failed to load world map data:', error);
      });
  }, [minCorrelation, selectedIndicator, selectedCountries]);

  const handleIndicatorChange = (event: any) => {
    setSelectedIndicator(event.target.value);
  };

  const handleCorrelationChange = (event: any, newValue: number | number[]) => {
    setMinCorrelation(newValue as number);
  };

  return (
    <Box sx={{ p: 3 }}>
      <Typography variant='h4' gutterBottom sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
        🌍 Interactive Global Economic Network Map
        <Tooltip title='Explore economic correlations between countries through an interactive network visualization'>
          <IconButton size='small'>
            <Info />
          </IconButton>
        </Tooltip>
      </Typography>

      {/* Controls */}
      <Paper sx={{ p: 2, mb: 3 }}>
        <Grid container spacing={3} alignItems='center'>
          <Grid item xs={12} md={3}>
            <FormControl fullWidth>
              <InputLabel>Economic Indicator</InputLabel>
              <Select
                value={selectedIndicator}
                label='Economic Indicator'
                onChange={handleIndicatorChange}
              >
                <MenuItem value='gdp'>GDP</MenuItem>
                <MenuItem value='inflation'>Inflation Rate</MenuItem>
                <MenuItem value='unemployment'>Unemployment Rate</MenuItem>
                <MenuItem value='trade'>Trade Balance</MenuItem>
              </Select>
            </FormControl>
          </Grid>

          <Grid item xs={12} md={3}>
            <FormControl fullWidth>
              <InputLabel>Region Filter</InputLabel>
              <Select
                value={selectedRegion}
                label='Region Filter'
                onChange={e => setSelectedRegion(e.target.value)}
              >
                <MenuItem value='all'>All Regions</MenuItem>
                {regions.map(region => (
                  <MenuItem key={region} value={region}>
                    {region}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>

          <Grid item xs={12} md={6}>
            <Autocomplete
              multiple
              options={filteredCountries}
              value={selectedCountries}
              onChange={(_, newValue) => setSelectedCountries(newValue)}
              groupBy={option => option.region || 'Other'}
              getOptionLabel={option => `${option.name} (${option.isoAlpha2})`}
              renderTags={(value, getTagProps) =>
                value.map((option, index) => (
                  <Chip
                    variant='outlined'
                    label={option.isoAlpha2}
                    {...getTagProps({ index })}
                    key={option.id}
                  />
                ))
              }
              renderInput={params => (
                <TextField
                  {...params}
                  label='Select Countries'
                  placeholder='Choose countries to analyze...'
                />
              )}
              renderGroup={params => (
                <li key={params.key}>
                  <ListSubheader component='div' sx={{ fontWeight: 'bold', color: 'primary.main' }}>
                    🌍 {params.group}
                  </ListSubheader>
                  {params.children}
                </li>
              )}
            />
          </Grid>
        </Grid>

        <Grid container spacing={3} alignItems='center' sx={{ mt: 1 }}>
          <Grid item xs={12} md={8}>
            <Typography gutterBottom>
              Minimum Correlation Threshold: {minCorrelation.toFixed(2)}
            </Typography>
            <Slider
              value={minCorrelation}
              onChange={handleCorrelationChange}
              min={0}
              max={1}
              step={0.05}
              valueLabelDisplay='auto'
              sx={{ mt: 1 }}
            />
          </Grid>

          <Grid item xs={12} md={4}>
            <Box sx={{ display: 'flex', gap: 1 }}>
              <Tooltip title='Zoom In'>
                <IconButton>
                  <ZoomIn />
                </IconButton>
              </Tooltip>
              <Tooltip title='Zoom Out'>
                <IconButton>
                  <ZoomOut />
                </IconButton>
              </Tooltip>
              <Tooltip title='Reset View'>
                <IconButton>
                  <RestartAlt />
                </IconButton>
              </Tooltip>
            </Box>
          </Grid>
        </Grid>
      </Paper>

      {/* Map Visualization */}
      <Paper sx={{ p: 2, mb: 3 }}>
        {loading ? (
          <Box sx={{ display: 'flex', justifyContent: 'center', p: 4 }}>
            <CircularProgress />
          </Box>
        ) : (
          <Box sx={{ display: 'flex', justifyContent: 'center' }}>
            <svg
              ref={svgRef}
              width={800}
              height={500}
              style={{ border: '1px solid #e0e0e0', borderRadius: '4px' }}
            />
          </Box>
        )}
      </Paper>

      {/* Selected Country Details */}
      {selectedCountry && (
        <Paper sx={{ p: 2 }}>
          <Typography variant='h6' gutterBottom>
            {selectedCountry.name} ({selectedCountry.isoAlpha2})
          </Typography>
          <Grid container spacing={2}>
            <Grid item xs={12} md={6}>
              <Typography variant='body2'>
                <strong>GDP:</strong> ${((selectedCountry.gdpUsd || 0) / 1e12).toFixed(2)}T
              </Typography>
              <Typography variant='body2'>
                <strong>Population:</strong> {((selectedCountry.population || 0) / 1e6).toFixed(0)}M
              </Typography>
            </Grid>
            <Grid item xs={12} md={6}>
              <Typography variant='body2'>
                <strong>Latitude:</strong> {selectedCountry.latitude.toFixed(2)}°
              </Typography>
              <Typography variant='body2'>
                <strong>Longitude:</strong> {selectedCountry.longitude.toFixed(2)}°
              </Typography>
            </Grid>
          </Grid>
        </Paper>
      )}

      {/* Legend */}
      <Paper sx={{ p: 2, mt: 2 }}>
        <Typography variant='h6' gutterBottom>
          Legend
        </Typography>
        <Box sx={{ display: 'flex', gap: 3, flexWrap: 'wrap' }}>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Box sx={{ width: 20, height: 20, backgroundColor: '#4CAF50', borderRadius: '50%' }} />
            <Typography variant='body2'>Countries (size = GDP)</Typography>
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Box sx={{ width: 30, height: 3, backgroundColor: '#2196F3' }} />
            <Typography variant='body2'>Positive Correlation</Typography>
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
            <Box sx={{ width: 30, height: 3, backgroundColor: '#F44336' }} />
            <Typography variant='body2'>Negative Correlation</Typography>
          </Box>
        </Box>
      </Paper>
    </Box>
  );
};

export default GlobalEconomicNetworkMap;
