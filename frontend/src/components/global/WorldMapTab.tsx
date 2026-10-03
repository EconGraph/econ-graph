/**
 * WorldMapTab Component.
 *
 * The Global Analysis page's map: pick a World Development Indicator and see each country's
 * latest value on a world map. Clicking a country opens its series.
 */

import React, { useCallback, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import {
  Alert,
  Box,
  Button,
  FormControl,
  Grid,
  InputLabel,
  LinearProgress,
  MenuItem,
  Paper,
  Select,
  Typography,
} from '@mui/material';
import InteractiveWorldMap from './InteractiveWorldMap';
import MapLegend from './MapLegend';
import MapValuesTable from './MapValuesTable';
import WorldMapControls from './WorldMapControls';
import { worldAtlasQuery } from './worldAtlas';
import { useCountryData } from './hooks/useCountryData';
import {
  DEFAULT_INDICATOR,
  useMapCrossSection,
  useMapDataset,
  type MapCountryValue,
} from './hooks/useWorldMapData';

const MAP_WIDTH = 960;
const MAP_HEIGHT = 500;

const NO_VALUES: MapCountryValue[] = [];

const WorldMapTab: React.FC = () => {
  const navigate = useNavigate();
  const [pickedIndicator, setPickedIndicator] = useState<string | null>(null);
  const [projection, setProjection] = useState('naturalEarth');
  const [colorScheme, setColorScheme] = useState('viridis');

  const datasetQuery = useMapDataset();
  const dataset = datasetQuery.data;
  const indicators = dataset?.indicators ?? [];

  // Until the user picks one: GDP per capita when the dataset has it, else its first indicator.
  const indicator =
    pickedIndicator ??
    (indicators.some(c => c.code === DEFAULT_INDICATOR)
      ? DEFAULT_INDICATOR
      : indicators[0]?.code) ??
    '';
  const indicatorLabel = indicators.find(c => c.code === indicator)?.label ?? indicator;

  const crossSectionQuery = useMapCrossSection(dataset, indicator);
  // While the next indicator loads, the map, legend and table keep showing the last one, and
  // name it from the data rather than from the picker.
  const shown = crossSectionQuery.data;
  const values = shown?.values ?? NO_VALUES;
  const unit = shown?.unit ?? null;
  const frequency = shown?.frequency ?? null;
  const shownLabel = indicators.find(c => c.code === shown?.indicator)?.label ?? shown?.indicator;

  // The outline the map draws, shared with the map through react-query's cache, for the codes
  // that have a shape.
  const atlasQuery = useQuery(worldAtlasQuery);
  const drawableIds = useMemo(() => {
    const geometries = atlasQuery.data?.objects.countries.geometries;
    if (!geometries) return undefined;
    return new Set(
      geometries.filter(g => g.id !== undefined && g.id !== '').map(g => Number(g.id))
    );
  }, [atlasQuery.data]);

  const {
    valuesByIsoNumeric,
    colorScale,
    dataRange,
    dateRange,
    countriesWithData,
    countriesWithoutData,
  } = useCountryData(values, colorScheme, drawableIds);

  // Stable, so the map's click handler never changes.
  const openSeries = useCallback(
    (country: MapCountryValue) => navigate(`/series/${country.seriesId}`),
    [navigate]
  );

  return (
    <Grid container spacing={3}>
      <Grid item xs={12} lg={9}>
        <Paper elevation={2} sx={{ p: 2 }}>
          <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center', mb: 2 }}>
            <FormControl size='small' sx={{ minWidth: 280, maxWidth: '100%' }} disabled={!dataset}>
              <InputLabel id='indicator-select-label'>Indicator</InputLabel>
              <Select
                labelId='indicator-select-label'
                value={indicator}
                label='Indicator'
                onChange={event => setPickedIndicator(event.target.value)}
                MenuProps={{ PaperProps: { sx: { maxHeight: 400 } } }}
              >
                {indicators.map(code => (
                  <MenuItem key={code.code} value={code.code}>
                    {code.label}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
            <WorldMapControls
              projection={projection}
              onProjectionChange={setProjection}
              colorScheme={colorScheme}
              onColorSchemeChange={setColorScheme}
            />
          </Box>

          {datasetQuery.isError && (
            <Alert
              severity='error'
              sx={{ mb: 2 }}
              action={
                <Button color='inherit' size='small' onClick={() => datasetQuery.refetch()}>
                  Retry
                </Button>
              }
            >
              Couldn't load the list of indicators.
            </Alert>
          )}
          {datasetQuery.isSuccess && !dataset && (
            <Alert severity='info' sx={{ mb: 2 }}>
              World Development Indicators haven't been loaded yet, so the map has no data.
            </Alert>
          )}
          {crossSectionQuery.isError && (
            <Alert
              severity='error'
              sx={{ mb: 2 }}
              action={
                <Button color='inherit' size='small' onClick={() => crossSectionQuery.refetch()}>
                  Retry
                </Button>
              }
            >
              Couldn't load {indicatorLabel}.
            </Alert>
          )}
          {crossSectionQuery.isSuccess &&
            !crossSectionQuery.isPreviousData &&
            countriesWithData.length === 0 && (
              <Alert severity='info' sx={{ mb: 2 }}>
                No country has a value for {indicatorLabel} yet.
              </Alert>
            )}

          <Box sx={{ height: 4, mb: 1 }}>
            {(datasetQuery.isFetching || crossSectionQuery.isFetching) && (
              <LinearProgress aria-label='Loading values' />
            )}
          </Box>
          <InteractiveWorldMap
            valuesByIsoNumeric={valuesByIsoNumeric}
            colorScale={colorScale}
            unit={unit}
            frequency={frequency}
            onCountryClick={openSeries}
            width={MAP_WIDTH}
            height={MAP_HEIGHT}
            projection={projection}
          />
          <Typography variant='caption' color='text.secondary' component='p' sx={{ mt: 1 }}>
            Hover over a country for its value; click it (tap twice on a touch screen) to open its
            series. Source: {dataset?.name ?? 'World Development Indicators'}, World Bank.
          </Typography>
          {values.length > 0 && (
            <MapValuesTable values={values} unit={unit} frequency={frequency} />
          )}
        </Paper>
      </Grid>
      <Grid item xs={12} lg={3}>
        {shown && shownLabel ? (
          <MapLegend
            colorScale={colorScale}
            indicator={shownLabel}
            unit={unit}
            frequency={frequency}
            dataRange={dataRange}
            dateRange={dateRange}
            countryCount={countriesWithData.length}
            undrawnCount={countriesWithoutData.length}
          />
        ) : (
          crossSectionQuery.isFetching && (
            <Paper variant='outlined' sx={{ p: 2 }}>
              <Typography variant='body2' color='text.secondary'>
                Loading {indicatorLabel}…
              </Typography>
            </Paper>
          )
        )}
      </Grid>
    </Grid>
  );
};

export default WorldMapTab;
