# Global Analysis Components Usage Guide

This guide provides comprehensive instructions for using the Global Analysis components in your React application.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Component Overview](#component-overview)
3. [Basic Usage](#basic-usage)
4. [Advanced Configuration](#advanced-configuration)
5. [Customization](#customization)
6. [Performance Optimization](#performance-optimization)
7. [Accessibility](#accessibility)
8. [Error Handling](#error-handling)
9. [Testing](#testing)
10. [Troubleshooting](#troubleshooting)

## Quick Start

### Installation

The Global Analysis components are already included in the project. No additional installation is required.

### Basic Setup

```tsx
import React, { useState, useCallback } from 'react';
import { EnhancedInteractiveWorldMap } from './components/global/EnhancedInteractiveWorldMap';
import { CountryData } from './types/globalAnalysis';
import { sampleCountryData } from './data/sampleCountryData';

const MyGlobalAnalysisPage: React.FC = () => {
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [selectedIndicator, setSelectedIndicator] = useState('GDP');
  const [mapView, setMapView] = useState({
    projection: 'naturalEarth',
    zoom: 1,
    center: [0, 0],
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
    <EnhancedInteractiveWorldMap
      data={sampleCountryData}
      selectedIndicator={selectedIndicator}
      timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
      onCountryClick={handleCountryClick}
      onCountryHover={handleCountryHover}
      mapView={mapView}
      onMapViewChange={setMapView}
      width={800}
      height={600}
      showControls={true}
      showLegend={true}
      showSelectionPanel={true}
      showStatisticalAnalysis={true}
    />
  );
};

export default MyGlobalAnalysisPage;
```

## Component Overview

### Core Components

- **`EnhancedInteractiveWorldMap`**: Main component that integrates all features
- **`MapControls`**: Zoom, projection, and display controls
- **`ColorLegend`**: Color scale and data coverage information
- **`CountrySelectionPanel`**: Selected countries management
- **`StatisticalAnalysis`**: Statistical analysis of selected countries

### Utility Components

- **`accessibility-utils.ts`**: ARIA labels, keyboard navigation
- **`performance-utils.ts`**: Performance optimization hooks
- **`error-handling-utils.ts`**: Error handling and loading states

## Basic Usage

### Required Props

```tsx
interface EnhancedInteractiveWorldMapProps {
  data: CountryData[];
  selectedIndicator: string;
  timeRange: { start: Date; end: Date };
  onCountryClick: (country: CountryData) => void;
  onCountryHover: (country: CountryData | null) => void;
  mapView: MapViewState;
  onMapViewChange: (view: MapViewState) => void;
  width: number;
  height: number;
}
```

### Optional Props

```tsx
interface EnhancedInteractiveWorldMapProps {
  // ... required props
  showControls?: boolean;
  showLegend?: boolean;
  showSelectionPanel?: boolean;
  showStatisticalAnalysis?: boolean;
  colorScheme?: 'viridis' | 'blues' | 'reds' | 'greens';
  showBorders?: boolean;
  showLabels?: boolean;
  labelSize?: number;
  onError?: (error: Error) => void;
  onLoadingChange?: (loading: boolean) => void;
}
```

## Advanced Configuration

### Custom Layout

```tsx
import { MapControls, ColorLegend, CountrySelectionPanel, StatisticalAnalysis } from './components/global';

const CustomLayout: React.FC = () => {
  return (
    <Grid container spacing={2}>
      <Grid item xs={12} md={8}>
        <EnhancedInteractiveWorldMap
          // ... props
          showControls={false}
          showLegend={false}
          showSelectionPanel={false}
          showStatisticalAnalysis={false}
        />
      </Grid>
      <Grid item xs={12} md={4}>
        <Stack spacing={2}>
          <MapControls
            zoomLevel={mapView.zoom}
            projection={mapView.projection}
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
          <ColorLegend
            indicator={selectedIndicator}
            minValue={0}
            maxValue={1000000}
            colorScheme="viridis"
            unit="USD"
            countriesWithData={data.length}
            countriesWithoutData={0}
          />
          <CountrySelectionPanel
            selectedCountries={selectedCountries}
            selectedIndicator={selectedIndicator}
            availableIndicators={['GDP', 'Inflation', 'Unemployment']}
            onRemoveCountry={handleRemoveCountry}
            onClearSelection={handleClearSelection}
            onCompareCountries={handleCompareCountries}
            onExportData={handleExportData}
            onIndicatorChange={handleIndicatorChange}
          />
          <StatisticalAnalysis
            selectedCountries={selectedCountries}
            selectedIndicator={selectedIndicator}
            onExportData={handleExportData}
          />
        </Stack>
      </Grid>
    </Grid>
  );
};
```

### Custom Styling

```tsx
const customStyles = {
  mapContainer: {
    border: '2px solid #1976d2',
    borderRadius: '8px',
    boxShadow: '0 4px 8px rgba(0,0,0,0.1)',
  },
  controls: {
    backgroundColor: '#f5f5f5',
    padding: '16px',
    borderRadius: '4px',
  },
  legend: {
    backgroundColor: '#ffffff',
    border: '1px solid #e0e0e0',
    borderRadius: '4px',
  },
};

<EnhancedInteractiveWorldMap
  // ... props
  sx={customStyles}
/>
```

## Customization

### Custom Color Schemes

```tsx
const customColorScheme = {
  name: 'Custom Blue',
  colors: ['#f7fbff', '#deebf7', '#c6dbef', '#9ecae1', '#6baed6', '#4292c6', '#2171b5', '#08519c', '#08306b'],
  type: 'sequential' as const,
};

<EnhancedInteractiveWorldMap
  // ... props
  colorScheme="custom"
  customColorScheme={customColorScheme}
/>
```

### Custom Projections

```tsx
const customProjections = [
  { name: 'Natural Earth', value: 'naturalEarth' },
  { name: 'Mercator', value: 'mercator' },
  { name: 'Orthographic', value: 'orthographic' },
  { name: 'Custom', value: 'custom' },
];

<MapControls
  // ... props
  projections={customProjections}
  onProjectionChange={handleCustomProjectionChange}
/>
```

### Custom Indicators

```tsx
const customIndicators = [
  { name: 'GDP', key: 'gdp', unit: 'USD', description: 'Gross Domestic Product' },
  { name: 'Inflation', key: 'inflation', unit: '%', description: 'Inflation Rate' },
  { name: 'Unemployment', key: 'unemployment', unit: '%', description: 'Unemployment Rate' },
];

<CountrySelectionPanel
  // ... props
  availableIndicators={customIndicators}
  onIndicatorChange={handleIndicatorChange}
/>
```

## Performance Optimization

### Memoization

```tsx
import { useMemoizedCountryData, useMemoizedColorScale } from './performance-utils';

const OptimizedComponent: React.FC = () => {
  const { processedData, countriesWithData, countriesWithoutData, dataRange } = useMemoizedCountryData(
    data,
    selectedIndicator,
    colorScheme
  );
  
  const colorScale = useMemoizedColorScale(dataRange, colorScheme);
  
  // ... rest of component
};
```

### Debouncing and Throttling

```tsx
import { debounce, throttle } from './performance-utils';

const handleZoomIn = useCallback(debounce(() => {
  setMapView(prev => ({ ...prev, zoom: prev.zoom * 1.2 }));
}, 100), []);

const handleZoomUpdate = useCallback(throttle(() => {
  setCurrentZoom(getZoomLevel());
}, 100), [getZoomLevel]);
```

### Memory Management

```tsx
import { memoryManagement } from './performance-utils';

useEffect(() => {
  return () => {
    if (svgRef.current) {
      memoryManagement.cleanupD3Selection(d3.select(svgRef.current));
    }
  };
}, []);
```

## Accessibility

### Keyboard Navigation

```tsx
import { keyboardHandlers, KEYBOARD_KEYS } from './accessibility-utils';

<Button
  onClick={handleAction}
  onKeyDown={(e) => keyboardHandlers.handleSelectionKeys(e, null, handleAction)}
  aria-label="Perform action"
>
  Action
</Button>
```

### Screen Reader Support

```tsx
import { ARIA_LABELS } from './accessibility-utils';

<Box
  role="region"
  aria-label={ARIA_LABELS.MAP_CONTAINER}
  aria-describedby="map-description"
>
  <Typography id="map-description" variant="caption" color="text.secondary">
    Interactive world map showing economic data for different countries
  </Typography>
</Box>
```

### High Contrast Mode

```tsx
const highContrastStyles = {
  country: {
    stroke: '#000000',
    strokeWidth: 2,
  },
  borders: {
    stroke: '#000000',
    strokeWidth: 2,
  },
  labels: {
    fill: '#000000',
    stroke: '#ffffff',
    strokeWidth: 1,
  },
};
```

## Error Handling

### Error Boundaries

```tsx
import { ErrorBoundary, ErrorDisplay } from './error-handling-utils';

<ErrorBoundary
  fallback={
    <ErrorDisplay
      error={createErrorContext(
        new Error('Map rendering failed'),
        ErrorType.RENDERING_ERROR,
        'MyComponent'
      )}
      onRetry={() => window.location.reload()}
    />
  }
>
  <EnhancedInteractiveWorldMap {...props} />
</ErrorBoundary>
```

### Error Handling Hook

```tsx
import { useErrorHandler } from './error-handling-utils';

const MyComponent: React.FC = () => {
  const { currentError, handleError, clearErrors } = useErrorHandler();
  
  const handleCountryClick = useCallback((country: CountryData) => {
    try {
      // ... country selection logic
    } catch (err) {
      handleError(err, ErrorType.RENDERING_ERROR, 'MyComponent', 'handleCountryClick');
    }
  }, [handleError]);
  
  return (
    <>
      <EnhancedInteractiveWorldMap {...props} />
      {currentError && (
        <ErrorSnackbar
          open={!!currentError}
          error={currentError}
          onClose={clearErrors}
          onRetry={() => { clearErrors(); /* retry logic */ }}
        />
      )}
    </>
  );
};
```

### Loading States

```tsx
import { LoadingState, EmptyState } from './error-handling-utils';

const MyComponent: React.FC = () => {
  if (isLoading) {
    return <LoadingState message="Loading world map..." size={60} />;
  }
  
  if (data.length === 0) {
    return <EmptyState message="No data available" />;
  }
  
  return <EnhancedInteractiveWorldMap {...props} />;
};
```

## Testing

### Unit Testing

```tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { setupD3Mocks } from '../../test-utils/d3-testing-utils';
import { EnhancedInteractiveWorldMap } from '../EnhancedInteractiveWorldMap';

describe('EnhancedInteractiveWorldMap', () => {
  beforeEach(() => {
    setupD3Mocks();
  });

  it('should render the map component', () => {
    render(<EnhancedInteractiveWorldMap {...mockProps} />);
    expect(screen.getByRole('region', { name: /interactive world map/i })).toBeInTheDocument();
  });

  it('should handle country clicks', () => {
    const onCountryClick = vi.fn();
    render(<EnhancedInteractiveWorldMap {...mockProps} onCountryClick={onCountryClick} />);
    
    // Simulate country click
    fireEvent.click(screen.getByTestId('country-usa'));
    expect(onCountryClick).toHaveBeenCalledWith(expect.objectContaining({ id: 'usa' }));
  });
});
```

### Integration Testing

```tsx
import { render, screen, waitFor } from '@testing-library/react';
import { GlobalAnalysisProvider } from '../../contexts/GlobalAnalysisContext';
import { EnhancedInteractiveWorldMap } from '../EnhancedInteractiveWorldMap';

describe('Global Analysis Integration', () => {
  it('should work with GlobalAnalysisProvider', async () => {
    render(
      <GlobalAnalysisProvider>
        <EnhancedInteractiveWorldMap {...mockProps} />
      </GlobalAnalysisProvider>
    );
    
    await waitFor(() => {
      expect(screen.getByRole('region', { name: /interactive world map/i })).toBeInTheDocument();
    });
  });
});
```

### Performance Testing

```tsx
import { performance } from 'perf_hooks';

describe('Performance Tests', () => {
  it('should render within performance budget', () => {
    const start = performance.now();
    render(<EnhancedInteractiveWorldMap {...mockProps} />);
    const end = performance.now();
    
    expect(end - start).toBeLessThan(100); // 100ms budget
  });
});
```

## Troubleshooting

### Common Issues

#### Map Not Displaying

**Problem**: The map doesn't render or shows a blank area.

**Solutions**:
1. Check that `worldData` is loaded correctly
2. Verify D3.js dependencies are installed
3. Check browser console for errors
4. Ensure proper SVG dimensions

```tsx
// Debug world data loading
console.log('World data:', worldData);
console.log('SVG dimensions:', { width, height });
```

#### Performance Issues

**Problem**: Map is slow or unresponsive.

**Solutions**:
1. Use memoization for expensive calculations
2. Implement debouncing for frequent updates
3. Optimize D3.js data binding
4. Check for memory leaks

```tsx
// Performance monitoring
const { renderCount, resetTimer } = usePerformanceMonitor('MyComponent');
console.log('Render count:', renderCount);
```

#### Accessibility Issues

**Problem**: Screen readers don't announce map information.

**Solutions**:
1. Add proper ARIA labels
2. Implement keyboard navigation
3. Provide text alternatives
4. Test with screen readers

```tsx
// Accessibility testing
import { axe, toHaveNoViolations } from 'jest-axe';
expect.extend(toHaveNoViolations);

it('should not have accessibility violations', async () => {
  const { container } = render(<EnhancedInteractiveWorldMap {...props} />);
  const results = await axe(container);
  expect(results).toHaveNoViolations();
});
```

### Debug Mode

```tsx
const DEBUG_MODE = process.env.NODE_ENV === 'development';

if (DEBUG_MODE) {
  console.log('Map state:', { mapView, selectedCountries, selectedIndicator });
  console.log('Performance metrics:', performanceMetrics);
}
```

### Error Reporting

```tsx
import { ErrorReporting } from './error-handling-utils';

const handleError = (error: Error) => {
  console.error('Map error:', error);
  ErrorReporting.reportError(error, {
    component: 'EnhancedInteractiveWorldMap',
    userAgent: navigator.userAgent,
    timestamp: new Date().toISOString(),
  });
};
```

## Best Practices

### Code Organization

1. **Separate concerns**: Keep map logic, data processing, and UI separate
2. **Use custom hooks**: Extract reusable logic into custom hooks
3. **Memoize expensive operations**: Use `useMemo` and `useCallback` appropriately
4. **Handle errors gracefully**: Implement proper error boundaries and fallbacks

### Performance

1. **Optimize re-renders**: Use React.memo for expensive components
2. **Debounce user interactions**: Prevent excessive API calls
3. **Clean up resources**: Remove event listeners and D3 selections
4. **Monitor performance**: Use performance monitoring tools

### Accessibility

1. **Provide alternatives**: Text descriptions for visual content
2. **Keyboard navigation**: All interactions should be keyboard accessible
3. **Screen reader support**: Proper ARIA labels and descriptions
4. **High contrast**: Ensure visibility in high contrast mode

### Testing

1. **Test user interactions**: Click, hover, keyboard navigation
2. **Test error states**: Loading, error, empty states
3. **Test accessibility**: Screen reader compatibility
4. **Test performance**: Render time and memory usage

## Support

For additional support or questions:

1. Check the component documentation
2. Review the test files for usage examples
3. Check the browser console for error messages
4. Verify all dependencies are installed correctly

## Changelog

### Version 1.0.0
- Initial release with basic map functionality
- Added map controls and legend
- Implemented country selection and statistical analysis
- Added accessibility features
- Performance optimizations
- Comprehensive error handling
- Full test coverage
