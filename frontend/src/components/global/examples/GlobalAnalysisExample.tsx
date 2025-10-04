/**
 * Global Analysis Example Component.
 *
 * Comprehensive example showing how to use the Global Analysis components
 * with various configurations and use cases.
 */

import React, { useState, useCallback } from 'react';
import {
  Box,
  Grid,
  Paper,
  Typography,
  Button,
  Chip,
  Stack,
  Divider,
  Alert,
  Snackbar,
} from '@mui/material';
import EnhancedInteractiveWorldMap from '../EnhancedInteractiveWorldMap';
import MapControls from '../MapControls';
import ColorLegend from '../ColorLegend';
import CountrySelectionPanel from '../CountrySelectionPanel';
import StatisticalAnalysis from '../StatisticalAnalysis';
import { CountryData, MapViewState } from '../../../types/globalAnalysis';
import { sampleCountryData } from '../../../data/sampleCountryData';

/**
 * Example 1: Basic Global Analysis Setup
 */
export const BasicGlobalAnalysisExample: React.FC = () => {
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [selectedIndicator, setSelectedIndicator] = useState('GDP');
  const [mapView, setMapView] = useState<MapViewState>({
    scale: 1,
    translation: [0, 0],
  });

  const handleCountryClick = useCallback((country: CountryData) => {
    setSelectedCountries(prev => {
      const isSelected = prev.some(c => c.id === country.id);
      if (isSelected) {
        return prev.filter(c => c.id !== country.id);
      } else {
        return [...prev, country];
      }
    });
  }, []);

  const handleCountryHover = useCallback((country: CountryData | null) => {
    // Handle hover logic
    console.log('Hovered country:', country);
  }, []);

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant='h4' gutterBottom>
        Basic Global Analysis Example
      </Typography>
      <Typography variant='body1' paragraph>
        This example shows the basic setup for the Global Analysis component with default settings
        and minimal configuration.
      </Typography>

      <EnhancedInteractiveWorldMap
        data={sampleCountryData}
        selectedIndicator={selectedIndicator}
        timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
        onCountryClick={handleCountryClick}
        onCountryHover={handleCountryHover}
        mapView={mapView}
        onMapViewChange={view =>
          setMapView(prev => ({ ...prev, ...(view as Partial<MapViewState>) }))
        }
        width={800}
        height={600}
        showControls={true}
        showLegend={true}
        showSelectionPanel={true}
        showStatisticalAnalysis={true}
      />
    </Box>
  );
};

/**
 * Example 2: Custom Layout with Separate Components
 */
export const CustomLayoutExample: React.FC = () => {
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [selectedIndicator, setSelectedIndicator] = useState('GDP');
  const [mapView, setMapView] = useState<MapViewState>({
    scale: 1,
    translation: [0, 0],
  });
  const [showBorders, setShowBorders] = useState(true);
  const [showLabels, setShowLabels] = useState(false);

  const handleCountryClick = useCallback((country: CountryData) => {
    setSelectedCountries(prev => {
      const isSelected = prev.some(c => c.id === country.id);
      if (isSelected) {
        return prev.filter(c => c.id !== country.id);
      } else {
        return [...prev, country];
      }
    });
  }, []);

  const handleCountryHover = useCallback((country: CountryData | null) => {
    console.log('Hovered country:', country);
  }, []);

  const handleRemoveCountry = useCallback((countryId: string) => {
    setSelectedCountries(prev => prev.filter(c => c.id !== countryId));
  }, []);

  const handleClearSelection = useCallback(() => {
    setSelectedCountries([]);
  }, []);

  const handleCompareCountries = useCallback(() => {
    console.log('Comparing countries:', selectedCountries);
  }, [selectedCountries]);

  const handleExportData = useCallback(() => {
    console.log('Exporting data for countries:', selectedCountries);
  }, [selectedCountries]);

  const handleIndicatorChange = useCallback((indicator: string) => {
    setSelectedIndicator(indicator);
  }, []);

  const handleZoomIn = useCallback(() => {
    setMapView(prev => ({ ...prev, scale: prev.scale * 1.2 }));
  }, []);

  const handleZoomOut = useCallback(() => {
    setMapView(prev => ({ ...prev, scale: prev.scale / 1.2 }));
  }, []);

  const handleResetZoom = useCallback(() => {
    setMapView(prev => ({ ...prev, scale: 1, translation: [0, 0] }));
  }, []);

  const handleProjectionChange = useCallback((_projection: string) => {
    // Projection handled internally in EnhancedInteractiveWorldMap
  }, []);

  const handleBordersToggle = useCallback((show: boolean) => {
    setShowBorders(show);
  }, []);

  const handleLabelsToggle = useCallback((show: boolean) => {
    setShowLabels(show);
  }, []);

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant='h4' gutterBottom>
        Custom Layout Example
      </Typography>
      <Typography variant='body1' paragraph>
        This example shows how to use individual components to create a custom layout with separate
        control panels and analysis sections.
      </Typography>

      <Grid container spacing={2}>
        <Grid item xs={12} md={8}>
          <Paper elevation={3} sx={{ p: 2 }}>
            <Typography variant='h6' gutterBottom>
              Interactive World Map
            </Typography>
            <EnhancedInteractiveWorldMap
              data={sampleCountryData}
              selectedIndicator={selectedIndicator}
              timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
              onCountryClick={handleCountryClick}
              onCountryHover={handleCountryHover}
              mapView={mapView}
              onMapViewChange={view =>
                setMapView(prev => ({ ...prev, ...(view as Partial<MapViewState>) }))
              }
              width={800}
              height={600}
              showControls={false}
              showLegend={false}
              showSelectionPanel={false}
              showStatisticalAnalysis={false}
            />
          </Paper>
        </Grid>

        <Grid item xs={12} md={4}>
          <Stack spacing={2}>
            <Paper elevation={3} sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                Map Controls
              </Typography>
              <MapControls
                zoomLevel={mapView.scale}
                projection={'naturalEarth'}
                projections={['naturalEarth', 'mercator', 'orthographic']}
                showBorders={showBorders}
                showLabels={showLabels}
                onZoomIn={handleZoomIn}
                onZoomOut={handleZoomOut}
                onResetZoom={handleResetZoom}
                onProjectionChange={handleProjectionChange}
                onBordersToggle={handleBordersToggle}
                onLabelsToggle={handleLabelsToggle}
              />
            </Paper>

            <Paper elevation={3} sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                Color Legend
              </Typography>
              <ColorLegend
                indicator={selectedIndicator}
                minValue={0}
                maxValue={1000000}
                colorScheme='viridis'
                unit='USD'
                countriesWithData={sampleCountryData.length}
                countriesWithoutData={0}
              />
            </Paper>

            <Paper elevation={3} sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                Selected Countries
              </Typography>
              <CountrySelectionPanel
                selectedCountries={selectedCountries}
                selectedIndicator={selectedIndicator}
                _availableIndicators={['GDP', 'Inflation', 'Unemployment']}
                onRemoveCountry={handleRemoveCountry}
                onClearSelection={handleClearSelection}
                onCompareCountries={handleCompareCountries}
                onExportData={handleExportData}
                _onIndicatorChange={handleIndicatorChange}
              />
            </Paper>

            <Paper elevation={3} sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                Statistical Analysis
              </Typography>
              <StatisticalAnalysis
                selectedCountries={selectedCountries}
                selectedIndicator={selectedIndicator}
                onExportData={handleExportData}
              />
            </Paper>
          </Stack>
        </Grid>
      </Grid>
    </Box>
  );
};

/**
 * Example 3: Error Handling and Loading States
 */
export const ErrorHandlingExample: React.FC = () => {
  const [hasError, setHasError] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [snackbarOpen, setSnackbarOpen] = useState(false);

  const handleSimulateError = useCallback(() => {
    setHasError(true);
    setSnackbarOpen(true);
  }, []);

  const handleSimulateLoading = useCallback(() => {
    setIsLoading(true);
    setTimeout(() => setIsLoading(false), 3000);
  }, []);

  const handleClearError = useCallback(() => {
    setHasError(false);
    setSnackbarOpen(false);
  }, []);

  if (hasError) {
    return (
      <Box sx={{ p: 2 }}>
        <Typography variant='h4' gutterBottom>
          Error Handling Example
        </Typography>
        <Alert
          severity='error'
          action={
            <Button color='inherit' size='small' onClick={handleClearError}>
              Retry
            </Button>
          }
        >
          An error occurred while loading the map. Please try again.
        </Alert>
      </Box>
    );
  }

  if (isLoading) {
    return (
      <Box sx={{ p: 2 }}>
        <Typography variant='h4' gutterBottom>
          Loading State Example
        </Typography>
        <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: 400 }}>
          <Box sx={{ textAlign: 'center' }}>
            <Typography variant='h6' gutterBottom>
              Loading Global Analysis...
            </Typography>
            <Typography variant='body2' color='text.secondary'>
              Please wait while we load the world map and economic data.
            </Typography>
          </Box>
        </Box>
      </Box>
    );
  }

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant='h4' gutterBottom>
        Error Handling Example
      </Typography>
      <Typography variant='body1' paragraph>
        This example demonstrates error handling and loading states in the Global Analysis
        component.
      </Typography>

      <Stack direction='row' spacing={2} sx={{ mb: 2 }}>
        <Button variant='outlined' onClick={handleSimulateError}>
          Simulate Error
        </Button>
        <Button variant='outlined' onClick={handleSimulateLoading}>
          Simulate Loading
        </Button>
      </Stack>

      <EnhancedInteractiveWorldMap
        data={sampleCountryData}
        selectedIndicator='GDP'
        timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
        onCountryClick={() => {}}
        onCountryHover={() => {}}
        mapView={{ scale: 1, translation: [0, 0] }}
        onMapViewChange={() => {}}
        width={800}
        height={600}
        showControls={true}
        showLegend={true}
        showSelectionPanel={true}
        showStatisticalAnalysis={true}
      />

      <Snackbar
        open={snackbarOpen}
        autoHideDuration={6000}
        onClose={handleClearError}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
      >
        <Alert onClose={handleClearError} severity='error' sx={{ width: '100%' }}>
          This is a simulated error for demonstration purposes.
        </Alert>
      </Snackbar>
    </Box>
  );
};

/**
 * Example 4: Accessibility Features
 */
export const AccessibilityExample: React.FC = () => {
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [selectedIndicator, setSelectedIndicator] = useState('GDP');
  const [mapView, setMapView] = useState<MapViewState>({
    scale: 1,
    translation: [0, 0],
  });

  const handleCountryClick = useCallback((country: CountryData) => {
    setSelectedCountries(prev => {
      const isSelected = prev.some(c => c.id === country.id);
      if (isSelected) {
        return prev.filter(c => c.id !== country.id);
      } else {
        return [...prev, country];
      }
    });
  }, []);

  const handleCountryHover = useCallback((country: CountryData | null) => {
    console.log('Hovered country:', country);
  }, []);

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant='h4' gutterBottom>
        Accessibility Features Example
      </Typography>
      <Typography variant='body1' paragraph>
        This example demonstrates the accessibility features of the Global Analysis component,
        including keyboard navigation, screen reader support, and ARIA labels.
      </Typography>

      <Alert severity='info' sx={{ mb: 2 }}>
        <Typography variant='body2'>
          <strong>Accessibility Features:</strong>
          <br />
          • Use Tab to navigate between controls
          <br />
          • Use Enter or Space to activate buttons
          <br />
          • Use Arrow keys to navigate country selections
          <br />
          • Screen readers will announce country information
          <br />• High contrast mode is supported
        </Typography>
      </Alert>

      <EnhancedInteractiveWorldMap
        data={sampleCountryData}
        selectedIndicator={selectedIndicator}
        timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
        onCountryClick={handleCountryClick}
        onCountryHover={handleCountryHover}
        mapView={mapView}
        onMapViewChange={view =>
          setMapView(prev => ({ ...prev, ...(view as Partial<MapViewState>) }))
        }
        width={800}
        height={600}
        showControls={true}
        showLegend={true}
        showSelectionPanel={true}
        showStatisticalAnalysis={true}
      />
    </Box>
  );
};

/**
 * Example 5: Performance Optimization
 */
export const PerformanceExample: React.FC = () => {
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [selectedIndicator, setSelectedIndicator] = useState('GDP');
  const [mapView, setMapView] = useState<MapViewState>({
    scale: 1,
    translation: [0, 0],
  });
  const [performanceMetrics, setPerformanceMetrics] = useState({
    renderCount: 0,
    lastRenderTime: 0,
  });

  const handleCountryClick = useCallback((country: CountryData) => {
    setSelectedCountries(prev => {
      const isSelected = prev.some(c => c.id === country.id);
      if (isSelected) {
        return prev.filter(c => c.id !== country.id);
      } else {
        return [...prev, country];
      }
    });
  }, []);

  const handleCountryHover = useCallback((country: CountryData | null) => {
    console.log('Hovered country:', country);
  }, []);

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant='h4' gutterBottom>
        Performance Optimization Example
      </Typography>
      <Typography variant='body1' paragraph>
        This example demonstrates the performance optimizations in the Global Analysis component,
        including memoization, debouncing, and efficient re-renders.
      </Typography>

      <Box sx={{ mb: 2, p: 2, bgcolor: 'grey.100', borderRadius: 1 }}>
        <Typography variant='h6' gutterBottom>
          Performance Metrics
        </Typography>
        <Stack direction='row' spacing={2}>
          <Chip label={`Renders: ${performanceMetrics.renderCount}`} color='primary' />
          <Chip label={`Last Render: ${performanceMetrics.lastRenderTime}ms`} color='secondary' />
        </Stack>
      </Box>

      <EnhancedInteractiveWorldMap
        data={sampleCountryData}
        selectedIndicator={selectedIndicator}
        timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
        onCountryClick={handleCountryClick}
        onCountryHover={handleCountryHover}
        mapView={mapView}
        onMapViewChange={view =>
          setMapView(prev => ({ ...prev, ...(view as Partial<MapViewState>) }))
        }
        width={800}
        height={600}
        showControls={true}
        showLegend={true}
        showSelectionPanel={true}
        showStatisticalAnalysis={true}
      />
    </Box>
  );
};

/**
 * Main Example Component
 */
export const GlobalAnalysisExamples: React.FC = () => {
  const [currentExample, setCurrentExample] = useState(0);

  const examples = [
    { title: 'Basic Setup', component: <BasicGlobalAnalysisExample /> },
    { title: 'Custom Layout', component: <CustomLayoutExample /> },
    { title: 'Error Handling', component: <ErrorHandlingExample /> },
    { title: 'Accessibility', component: <AccessibilityExample /> },
    { title: 'Performance', component: <PerformanceExample /> },
  ];

  return (
    <Box sx={{ p: 2 }}>
      <Typography variant='h3' gutterBottom>
        Global Analysis Examples
      </Typography>
      <Typography variant='body1' paragraph>
        This page demonstrates various ways to use the Global Analysis components with different
        configurations and use cases.
      </Typography>

      <Stack direction='row' spacing={1} sx={{ mb: 3 }}>
        {examples.map((example, index) => (
          <Button
            key={index}
            variant={currentExample === index ? 'contained' : 'outlined'}
            onClick={() => setCurrentExample(index)}
          >
            {example.title}
          </Button>
        ))}
      </Stack>

      <Divider sx={{ mb: 3 }} />

      {examples[currentExample].component}
    </Box>
  );
};

export default GlobalAnalysisExamples;
