/**
 * EnhancedInteractiveWorldMap Component.
 *
 * An enhanced D3.js-powered interactive world map component with comprehensive
 * controls, legend, country selection, and statistical analysis features.
 */

import React, { useRef, useEffect, Suspense, useState, useCallback, useMemo } from 'react';
import * as d3 from 'd3';
import * as topojson from 'topojson-client';
import { Box, CircularProgress, Grid, Paper, Typography, Alert, Snackbar } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { CountryData, MapViewState } from '../../types/globalAnalysis';
import { useWorldMap } from './hooks/useWorldMap';
import { useCountryData } from './hooks/useCountryData';
import MapControls from './MapControls';
import ColorLegend from './ColorLegend';
import CountrySelectionPanel from './CountrySelectionPanel';
import StatisticalAnalysis from './StatisticalAnalysis';
import {
  useMemoizedCountryData,
  useMemoizedColorScale,
  useMemoizedStatistics,
  usePerformanceMonitor,
  debounce,
  throttle,
  memoryManagement,
  d3PerformanceOptimizations,
} from './performance-utils';
import {
  ErrorBoundary,
  ErrorDisplay,
  LoadingState,
  EmptyState,
  ErrorSnackbar,
  ErrorDialog,
  useErrorHandler,
  ErrorType,
  createErrorContext,
} from './error-handling-utils';

interface EnhancedInteractiveWorldMapProps {
  /** Array of country data to display on the map. */
  data: CountryData[];
  /** Currently selected economic indicator. */
  selectedIndicator: string;
  /** Time range for data visualization. */
  timeRange: { start: Date; end: Date };
  /** Callback when a country is clicked. */
  onCountryClick: (country: CountryData) => void;
  /** Callback when a country is hovered. */
  onCountryHover: (country: CountryData | null) => void;
  /** Current map view state. */
  mapView: MapViewState;
  /** Callback when map view changes. */
  onMapViewChange: (view: Partial<MapViewState>) => void;
  /** Whether animation is enabled. */
  animationEnabled?: boolean;
  /** Whether to show country borders. */
  showBorders?: boolean;
  /** Whether to show country labels. */
  showLabels?: boolean;
  /** Size of country labels. */
  labelSize?: number;
  /** Width of the map container. */
  width: number;
  /** Height of the map container. */
  height: number;
  /** Map projection type. */
  projection?: string;
  /** Color scheme for data visualization. */
  colorScheme?: string;
  /** Whether to show controls panel. */
  showControls?: boolean;
  /** Whether to show legend. */
  showLegend?: boolean;
  /** Whether to show country selection panel. */
  showSelectionPanel?: boolean;
  /** Whether to show statistical analysis. */
  showStatisticalAnalysis?: boolean;
}

// Fetch world atlas data using React Query
const useWorldAtlasData = () => {
  return useQuery({
    queryKey: ['world-atlas'],
    queryFn: async () => {
      const response = await fetch('https://cdn.jsdelivr.net/npm/world-atlas@3/world/110m.json');
      if (!response.ok) {
        throw new Error('Failed to load world map data');
      }
      return response.json();
    },
    staleTime: Infinity, // Atlas data never changes
    cacheTime: Infinity,
    suspense: true,
  });
};

// Main map content component - assumes data is loaded
const EnhancedWorldMapContent: React.FC<EnhancedInteractiveWorldMapProps> = ({
  data,
  selectedIndicator,
  timeRange: _timeRange,
  onCountryClick,
  onCountryHover,
  mapView: _mapView,
  onMapViewChange: _onMapViewChange,
  animationEnabled: _animationEnabled = true,
  showBorders = true,
  showLabels = false,
  labelSize = 12,
  width,
  height,
  projection = 'naturalEarth',
  colorScheme = 'viridis',
  showControls = true,
  showLegend = true,
  showSelectionPanel = true,
  showStatisticalAnalysis = true,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [currentZoom, setCurrentZoom] = useState(1);
  const [error, setError] = useState<string | null>(null);
  const [snackbarOpen, setSnackbarOpen] = useState(false);
  const [showErrorDialog, setShowErrorDialog] = useState(false);

  // Error handling
  const { currentError, handleError, clearErrors } = useErrorHandler();

  // Performance monitoring
  const { renderCount, resetTimer } = usePerformanceMonitor('EnhancedWorldMapContent');

  // Load world atlas data with Suspense
  const { data: worldData } = useWorldAtlasData();

  // Custom hooks for map logic
  const {
    path,
    zoomBehavior,
    zoomToFit,
    zoomToCountry,
    resetZoom,
    getZoomLevel,
    getCenter,
    projections,
  } = useWorldMap(svgRef, projection);

  // Memoized data processing for performance
  const { processedData, countriesWithData, countriesWithoutData, dataRange } =
    useMemoizedCountryData(data, selectedIndicator, colorScheme);

  // Memoized color scale for performance
  const colorScale = useMemoizedColorScale(dataRange, colorScheme);

  // Memoized statistics for performance
  const statistics = useMemoizedStatistics(selectedCountries, selectedIndicator);

  // Available indicators (memoized)
  const availableIndicators = useMemo(() => {
    const indicators = new Set<string>();
    data.forEach(country => {
      country.economicIndicators?.forEach(indicator => {
        indicators.add(indicator.name);
      });
    });
    return Array.from(indicators);
  }, [data]);

  // Handle country selection (memoized for performance)
  const handleCountryClick = useCallback(
    (country: CountryData) => {
      try {
        setSelectedCountries(prev => {
          const isSelected = prev.some(c => c.id === country.id);
          if (isSelected) {
            return prev.filter(c => c.id !== country.id);
          } else {
            if (prev.length >= 10) {
              const errorContext = createErrorContext(
                new Error('Maximum 10 countries can be selected'),
                ErrorType.VALIDATION_ERROR,
                'EnhancedWorldMapContent',
                'handleCountryClick'
              );
              handleError(errorContext);
              return prev;
            }
            return [...prev, country];
          }
        });
        onCountryClick(country);
      } catch (err) {
        handleError(
          err,
          ErrorType.RENDERING_ERROR,
          'EnhancedWorldMapContent',
          'handleCountryClick'
        );
      }
    },
    [onCountryClick, handleError]
  );

  // Debounced zoom handlers for performance
  const debouncedZoomIn = useCallback(
    debounce(() => {
      console.log('Zooming in');
    }, 100),
    []
  );

  const debouncedZoomOut = useCallback(
    debounce(() => {
      console.log('Zooming out');
    }, 100),
    []
  );

  // Throttled zoom level updates for performance
  const throttledZoomUpdate = useCallback(
    throttle(() => {
      setCurrentZoom(getZoomLevel());
    }, 100),
    [getZoomLevel]
  );

  // Handle country removal
  const handleRemoveCountry = useCallback((countryId: string) => {
    setSelectedCountries(prev => prev.filter(c => c.id !== countryId));
  }, []);

  // Handle clear selection
  const handleClearSelection = useCallback(() => {
    setSelectedCountries([]);
  }, []);

  // Handle country comparison
  const handleCompareCountries = useCallback(() => {
    if (selectedCountries.length >= 2) {
      // TODO: Implement country comparison modal
      console.log('Comparing countries:', selectedCountries);
    }
  }, [selectedCountries]);

  // Handle data export
  const handleExportData = useCallback(() => {
    // TODO: Implement data export
    console.log('Exporting data for countries:', selectedCountries);
  }, [selectedCountries]);

  // Handle indicator change
  const handleIndicatorChange = useCallback((indicator: string) => {
    // TODO: Implement indicator change
    console.log('Changing indicator to:', indicator);
  }, []);

  // Handle zoom controls
  const handleZoomIn = useCallback(() => {
    // TODO: Implement zoom in
    console.log('Zooming in');
  }, []);

  const handleZoomOut = useCallback(() => {
    // TODO: Implement zoom out
    console.log('Zooming out');
  }, []);

  const handleResetZoom = useCallback(() => {
    resetZoom();
  }, [resetZoom]);

  // Handle projection change
  const handleProjectionChange = useCallback((newProjection: string) => {
    // TODO: Implement projection change
    console.log('Changing projection to:', newProjection);
  }, []);

  // Handle border toggle
  const handleBordersToggle = useCallback((show: boolean) => {
    // TODO: Implement border toggle
    console.log('Toggling borders:', show);
  }, []);

  // Handle labels toggle
  const handleLabelsToggle = useCallback((show: boolean) => {
    // TODO: Implement labels toggle
    console.log('Toggling labels:', show);
  }, []);

  // Initialize D3 map with performance optimizations and error handling
  useEffect(() => {
    if (!worldData || !svgRef.current) return;

    try {
      const svg = d3.select(svgRef.current);

      // Performance optimization: Only clear and redraw if necessary
      const needsRedraw = svg.select('.map-container').empty();

      if (needsRedraw) {
        svg.selectAll('*').remove(); // Clear previous content

        // Set up SVG dimensions
        svg
          .attr('width', width)
          .attr('height', height)
          .attr('viewBox', `0 0 ${width} ${height}`)
          .style('background-color', '#f5f5f5')
          .style('border-radius', '8px')
          .style('box-shadow', '0 2px 8px rgba(0,0,0,0.1)');

        // Create map container
        const mapContainer = svg.append('g').attr('class', 'map-container');

        // Create countries group
        const countriesGroup = mapContainer.append('g').attr('class', 'countries');

        // Create borders group
        const bordersGroup = mapContainer.append('g').attr('class', 'borders');

        // Render world map
        const countries = worldData.objects.countries;
        const countriesPath = topojson.feature(worldData, countries) as any;

        // Performance optimization: Use D3's data binding for efficient updates
        const { update, enter, exit } = d3PerformanceOptimizations.efficientDataBinding(
          countriesGroup.selectAll('path.country'),
          countriesPath.features,
          (d: any) => d.properties.ISO_A2
        );

        // Remove exiting elements
        exit.remove();

        // Add new elements
        const newPaths = enter.append('path').attr('class', 'country').style('cursor', 'pointer');

        // Update existing elements
        update
          .merge(newPaths)
          .attr('d', (d: any) => path(d))
          .style('fill', (d: any) => {
            const countryCode = d.properties.ISO_A2;
            const countryData = processedData.find(c => c.isoAlpha2 === countryCode);

            if (countryData && countryData.economicIndicators) {
              const indicator = countryData.economicIndicators.find(
                ind => ind.name === selectedIndicator
              );
              return indicator ? colorScale(indicator.value) : '#d0d0d0';
            }
            return '#d0d0d0';
          })
          .style('stroke', showBorders ? '#ffffff' : '#cccccc')
          .style('stroke-width', showBorders ? '1' : '0.5')
          .style('opacity', '0.9')
          .on('click', (event: MouseEvent, d: any) => {
            try {
              const countryCode = d.properties.ISO_A2;
              const countryData = processedData.find(c => c.isoAlpha2 === countryCode);
              if (countryData) {
                handleCountryClick(countryData);
              }
            } catch (err) {
              handleError(
                err,
                ErrorType.RENDERING_ERROR,
                'EnhancedWorldMapContent',
                'countryClick'
              );
            }
          })
          .on('mouseover', (event: MouseEvent, d: any) => {
            try {
              const countryCode = d.properties.ISO_A2;
              const countryData = processedData.find(c => c.isoAlpha2 === countryCode);
              if (countryData) {
                onCountryHover(countryData);
                d3.select(event.currentTarget as SVGElement)
                  .style('stroke', '#1976d2')
                  .style('stroke-width', '3')
                  .style('opacity', '1')
                  .style('filter', 'drop-shadow(0 2px 4px rgba(0,0,0,0.3))');
              }
            } catch (err) {
              handleError(
                err,
                ErrorType.RENDERING_ERROR,
                'EnhancedWorldMapContent',
                'countryHover'
              );
            }
          })
          .on('mouseout', (event: MouseEvent) => {
            try {
              onCountryHover(null);
              d3.select(event.currentTarget as SVGElement)
                .style('stroke', showBorders ? '#ffffff' : '#cccccc')
                .style('stroke-width', showBorders ? '1' : '0.5')
                .style('opacity', '0.9')
                .style('filter', 'none');
            } catch (err) {
              handleError(
                err,
                ErrorType.RENDERING_ERROR,
                'EnhancedWorldMapContent',
                'countryMouseOut'
              );
            }
          });

        // Draw country borders
        if (showBorders) {
          bordersGroup
            .append('path')
            .datum(topojson.mesh(worldData, countries, (a, b) => a !== b))
            .attr('class', 'border')
            .attr('d', (d: any) => path(d))
            .style('fill', 'none')
            .style('stroke', '#ffffff')
            .style('stroke-width', '1')
            .style('opacity', '0.8');
        }

        // Add country labels
        if (showLabels) {
          countriesGroup
            .selectAll('text.country-label')
            .data(countriesPath.features)
            .enter()
            .append('text')
            .attr('class', 'country-label')
            .attr('x', (d: any) => {
              const centroid = path.centroid(d);
              return centroid[0];
            })
            .attr('y', (d: any) => {
              const centroid = path.centroid(d);
              return centroid[1];
            })
            .attr('text-anchor', 'middle')
            .attr('font-size', labelSize)
            .attr('font-family', 'Arial, sans-serif')
            .attr('fill', '#2c3e50')
            .attr('font-weight', '500')
            .text((d: any) => d.properties.NAME)
            .style('pointer-events', 'none')
            .style('text-shadow', '1px 1px 2px rgba(255,255,255,0.8)');
        }

        // Set up zoom behavior
        svg.call(zoomBehavior);
      }

      // Update zoom level with throttling
      throttledZoomUpdate();
    } catch (err) {
      handleError(err, ErrorType.RENDERING_ERROR, 'EnhancedWorldMapContent', 'initializeMap');
    }
  }, [
    worldData,
    processedData,
    selectedIndicator,
    colorScale,
    width,
    height,
    showBorders,
    showLabels,
    labelSize,
    path,
    zoomBehavior,
    onCountryHover,
    handleCountryClick,
    throttledZoomUpdate,
    handleError,
  ]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (svgRef.current) {
        memoryManagement.cleanupD3Selection(d3.select(svgRef.current));
      }
    };
  }, []);

  return (
    <Box sx={{ width: '100%', height: '100%' }}>
      <Grid container spacing={2} sx={{ height: '100%' }}>
        {/* Left Panel - Controls and Legend */}
        <Grid item xs={12} md={3}>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, height: '100%' }}>
            {showControls && (
              <MapControls
                zoomLevel={currentZoom}
                projection={projection}
                projections={projections}
                showBorders={showBorders}
                showLabels={showLabels}
                onZoomIn={handleZoomIn}
                onZoomOut={handleZoomOut}
                onResetZoom={handleResetZoom}
                onProjectionChange={handleProjectionChange}
                onBordersToggle={handleBordersToggle}
                onLabelsToggle={handleLabelsToggle}
              />
            )}

            {showLegend && (
              <ColorLegend
                indicator={selectedIndicator}
                minValue={dataRange.min}
                maxValue={dataRange.max}
                colorScheme={colorScheme}
                unit={processedData[0]?.economicIndicators?.[0]?.unit || ''}
                countriesWithData={countriesWithData.length}
                countriesWithoutData={countriesWithoutData.length}
              />
            )}
          </Box>
        </Grid>

        {/* Center Panel - Map */}
        <Grid item xs={12} md={6}>
          <Box
            width={width}
            height={height}
            border='2px solid #e0e0e0'
            borderRadius={2}
            overflow='hidden'
            boxShadow='0 4px 12px rgba(0,0,0,0.15)'
            bgcolor='#fafafa'
          >
            <svg
              ref={svgRef}
              width={width}
              height={height}
              style={{
                display: 'block',
                borderRadius: '6px',
              }}
            />
          </Box>
        </Grid>

        {/* Right Panel - Selection and Analysis */}
        <Grid item xs={12} md={3}>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, height: '100%' }}>
            {showSelectionPanel && (
              <CountrySelectionPanel
                selectedCountries={selectedCountries}
                selectedIndicator={selectedIndicator}
                availableIndicators={availableIndicators}
                onRemoveCountry={handleRemoveCountry}
                onClearSelection={handleClearSelection}
                onCompareCountries={handleCompareCountries}
                onExportData={handleExportData}
                onIndicatorChange={handleIndicatorChange}
              />
            )}

            {showStatisticalAnalysis && (
              <StatisticalAnalysis
                selectedCountries={selectedCountries}
                selectedIndicator={selectedIndicator}
                onExportData={handleExportData}
              />
            )}
          </Box>
        </Grid>
      </Grid>

      {/* Error Snackbar */}
      {currentError && (
        <ErrorSnackbar
          open={!!currentError}
          error={currentError}
          onClose={() => clearErrors()}
          onRetry={() => {
            clearErrors();
            // Retry logic here
          }}
        />
      )}

      {/* Error Dialog */}
      {currentError && (
        <ErrorDialog
          open={showErrorDialog}
          error={currentError}
          onClose={() => setShowErrorDialog(false)}
          onRetry={() => {
            clearErrors();
            setShowErrorDialog(false);
            // Retry logic here
          }}
        />
      )}
    </Box>
  );
};

// Wrapper component with Suspense boundary and Error Boundary
const EnhancedInteractiveWorldMap: React.FC<EnhancedInteractiveWorldMapProps> = props => {
  return (
    <ErrorBoundary
      fallback={
        <Box
          display='flex'
          justifyContent='center'
          alignItems='center'
          height={props.height}
          width={props.width}
          p={2}
        >
          <ErrorDisplay
            error={createErrorContext(
              new Error('Map rendering failed'),
              ErrorType.RENDERING_ERROR,
              'EnhancedInteractiveWorldMap'
            )}
            onRetry={() => window.location.reload()}
          />
        </Box>
      }
    >
      <Suspense
        fallback={
          <Box
            display='flex'
            justifyContent='center'
            alignItems='center'
            height={props.height}
            width={props.width}
          >
            <LoadingState message='Loading world map...' size={60} />
          </Box>
        }
      >
        <EnhancedWorldMapContent {...props} />
      </Suspense>
    </ErrorBoundary>
  );
};

export default EnhancedInteractiveWorldMap;
