# Global Analysis Components

This directory contains the enhanced Global Analysis components for the EconGraph application, featuring interactive world maps, statistical analysis, and comprehensive data visualization capabilities.

## Overview

The Global Analysis feature provides powerful tools for visualizing and analyzing economic data across countries. The components are built with React, TypeScript, Material-UI, and D3.js, with a focus on accessibility, performance, and user experience.

## Components

### Core Components

#### `EnhancedInteractiveWorldMap`
The main component that orchestrates all Global Analysis functionality.

**Features:**
- Interactive D3.js world map with zoom and pan
- Multiple map projections (Natural Earth, Mercator, Orthographic)
- Country selection and data visualization
- Real-time statistical analysis
- Comprehensive error handling
- Accessibility support
- Performance optimizations

**Props:**
```typescript
interface EnhancedInteractiveWorldMapProps {
  data: CountryData[];
  selectedIndicator: string;
  timeRange: { start: Date; end: Date };
  onCountryClick: (country: CountryData) => void;
  onCountryHover: (country: CountryData | null) => void;
  mapView: MapViewState;
  onMapViewChange: (view: Partial<MapViewState>) => void;
  animationEnabled?: boolean;
  showBorders?: boolean;
  showLabels?: boolean;
  labelSize?: number;
  width: number;
  height: number;
  projection?: string;
  colorScheme?: string;
  showControls?: boolean;
  showLegend?: boolean;
  showSelectionPanel?: boolean;
  showStatisticalAnalysis?: boolean;
}
```

#### `MapControls`
Control panel for map navigation and settings.

**Features:**
- Zoom controls (in, out, reset)
- Projection selector
- Display options (borders, labels)
- Quick actions
- Accessibility support

#### `ColorLegend`
Color scale legend showing data ranges and mapping.

**Features:**
- Dynamic color scales
- Value range display
- Data coverage information
- Color scheme information
- Accessibility support

#### `CountrySelectionPanel`
Sidebar for managing selected countries.

**Features:**
- Country list with sorting and filtering
- Selection management
- Export functionality
- Comparison tools
- Accessibility support

#### `StatisticalAnalysis`
Statistical analysis panel for selected countries.

**Features:**
- Descriptive statistics
- Value range analysis
- Top/bottom countries
- Regional distribution
- Data quality metrics
- Accessibility support

### Utility Components

#### `ErrorBoundary`
React error boundary for catching and handling errors gracefully.

#### `ErrorDisplay`
Component for displaying error messages with retry options.

#### `LoadingState`
Loading indicator with progress support.

#### `EmptyState`
Empty state display for when no data is available.

## Usage Examples

### Basic Usage

```typescript
import { EnhancedInteractiveWorldMap } from './components/global/EnhancedInteractiveWorldMap';
import { CountryData } from './types/globalAnalysis';

const GlobalAnalysisPage = () => {
  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [selectedIndicator, setSelectedIndicator] = useState('GDP');
  const [mapView, setMapView] = useState({
    projection: 'naturalEarth',
    zoom: 1,
    center: [0, 0]
  });

  const handleCountryClick = (country: CountryData) => {
    setSelectedCountries(prev => [...prev, country]);
  };

  const handleCountryHover = (country: CountryData | null) => {
    // Handle hover logic
  };

  return (
    <EnhancedInteractiveWorldMap
      data={countryData}
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
```

### Advanced Usage with Custom Controls

```typescript
import { 
  EnhancedInteractiveWorldMap,
  MapControls,
  ColorLegend,
  CountrySelectionPanel,
  StatisticalAnalysis
} from './components/global';

const CustomGlobalAnalysis = () => {
  const [mapState, setMapState] = useState({
    projection: 'naturalEarth',
    zoom: 1,
    showBorders: true,
    showLabels: false
  });

  const [selectedCountries, setSelectedCountries] = useState<CountryData[]>([]);
  const [selectedIndicator, setSelectedIndicator] = useState('GDP');

  return (
    <Grid container spacing={2}>
      <Grid item xs={12} md={8}>
        <EnhancedInteractiveWorldMap
          data={countryData}
          selectedIndicator={selectedIndicator}
          timeRange={timeRange}
          onCountryClick={handleCountryClick}
          onCountryHover={handleCountryHover}
          mapView={mapState}
          onMapViewChange={setMapState}
          width={800}
          height={600}
          showControls={false}
          showLegend={false}
          showSelectionPanel={false}
          showStatisticalAnalysis={false}
        />
      </Grid>
      <Grid item xs={12} md={4}>
        <Stack spacing={2}>
          <MapControls
            zoomLevel={mapState.zoom}
            projection={mapState.projection}
            projections={['naturalEarth', 'mercator', 'orthographic']}
            showBorders={mapState.showBorders}
            showLabels={mapState.showLabels}
            onZoomIn={handleZoomIn}
            onZoomOut={handleZoomOut}
            onResetZoom={handleResetZoom}
            onProjectionChange={handleProjectionChange}
            onBordersToggle={handleBordersToggle}
            onLabelsToggle={handleLabelsToggle}
          />
          <ColorLegend
            indicator={selectedIndicator}
            minValue={dataRange.min}
            maxValue={dataRange.max}
            colorScheme="viridis"
            unit="USD"
            countriesWithData={countriesWithData.length}
            countriesWithoutData={countriesWithoutData.length}
          />
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

## Hooks

### `useWorldMap`
Custom hook for managing D3.js map logic.

```typescript
const { path, zoomBehavior, resetZoom, getZoomLevel } = useWorldMap(svgRef, 'naturalEarth');
```

### `useCountryData`
Custom hook for processing country data and creating color scales.

```typescript
const { processedData, colorScale, dataRange } = useCountryData(countries, 'GDP', 'viridis');
```

### `useErrorHandler`
Custom hook for error handling.

```typescript
const { currentError, handleError, clearErrors } = useErrorHandler();
```

## Utilities

### Performance Utilities
- `useMemoizedCountryData`: Memoized country data processing
- `useMemoizedColorScale`: Memoized color scale generation
- `useMemoizedStatistics`: Memoized statistical calculations
- `debounce`: Debounce utility for performance
- `throttle`: Throttle utility for performance

### Accessibility Utilities
- `ARIA_LABELS`: Predefined ARIA labels
- `getCountryDescription`: Generate accessible country descriptions
- `getColorLegendDescription`: Generate accessible legend descriptions
- `keyboardHandlers`: Keyboard navigation utilities

### Error Handling Utilities
- `ErrorBoundary`: React error boundary
- `ErrorDisplay`: Error display component
- `LoadingState`: Loading state component
- `EmptyState`: Empty state component
- `useErrorHandler`: Error handling hook

## Testing

### Unit Tests
```bash
npm test -- --testPathPattern=global
```

### Integration Tests
```bash
npm test -- --testPathPattern=GlobalAnalysis
```

### E2E Tests
```bash
npm run test:e2e -- --grep "Global Analysis"
```

## Accessibility

The components are built with accessibility in mind:

- **ARIA Labels**: All interactive elements have proper ARIA labels
- **Keyboard Navigation**: Full keyboard support for all interactions
- **Screen Reader Support**: Comprehensive screen reader announcements
- **High Contrast**: Support for high contrast mode
- **Reduced Motion**: Respects user's motion preferences

## Performance

The components are optimized for performance:

- **Memoization**: Extensive use of React.memo and useMemo
- **Debouncing**: Debounced event handlers for smooth interactions
- **Throttling**: Throttled updates for real-time data
- **Virtual Scrolling**: For large datasets
- **Memory Management**: Proper cleanup of D3.js selections

## Error Handling

Comprehensive error handling includes:

- **Error Boundaries**: Catch React errors gracefully
- **Error Types**: Categorized error types with appropriate handling
- **User-Friendly Messages**: Clear, actionable error messages
- **Retry Logic**: Automatic and manual retry options
- **Error Reporting**: Detailed error context for debugging

## Browser Support

- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

## Dependencies

- React 18+
- TypeScript 4.9+
- Material-UI 5+
- D3.js 7+
- @tanstack/react-query 4+

## Contributing

When contributing to the Global Analysis components:

1. Follow the established patterns for component structure
2. Add comprehensive tests for new functionality
3. Ensure accessibility compliance
4. Optimize for performance
5. Add proper error handling
6. Update documentation

## License

This project is licensed under the MIT License.
