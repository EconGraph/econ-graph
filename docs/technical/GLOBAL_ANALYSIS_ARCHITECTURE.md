# Global Analysis Technical Architecture

## Overview

The Global Analysis feature provides interactive visualization of economic data across countries using D3.js-powered world maps, React Context API for state management, and Material-UI for responsive design. This document details the technical architecture, component structure, and implementation patterns.

## Table of Contents

- [System Architecture](#system-architecture)
- [Component Architecture](#component-architecture)
- [State Management](#state-management)
- [Data Flow](#data-flow)
- [D3.js Integration](#d3js-integration)
- [Type System](#type-system)
- [Testing Strategy](#testing-strategy)
- [Performance Considerations](#performance-considerations)
- [Accessibility](#accessibility)
- [Future Enhancements](#future-enhancements)

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Global Analysis UI                       │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │   React App     │  │   D3.js Maps    │  │  Material-UI │ │
│  │   Components    │  │   Visualizations│  │  Components  │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │  Context API    │  │   Custom Hooks  │  │  Type System │ │
│  │  State Mgmt     │  │   D3 Logic      │  │  TypeScript  │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │   Sample Data   │  │   World Atlas   │  │  Test Suite  │ │
│  │   Mock Data     │  │   TopoJSON      │  │  Jest/Playwright│
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Technology Stack

- **Frontend Framework**: React 18 with TypeScript
- **Visualization**: D3.js v7 with d3-geo, d3-scale, d3-zoom
- **UI Components**: Material-UI (MUI) v5
- **State Management**: React Context API
- **Data Format**: TopoJSON for world map data
- **Testing**: Jest + React Testing Library + Playwright
- **Build Tools**: Create React App with custom Jest configuration

## Component Architecture

### Component Hierarchy

```
GlobalAnalysisDemo
├── InteractiveWorldMap
│   ├── CountryTooltip
│   └── MapLegend
├── WorldMapControls
└── SelectedCountriesList
```

### Core Components

#### 1. InteractiveWorldMap

**Purpose**: Main D3.js-powered world map component

**Key Features**:
- Multiple map projections (Natural Earth, Mercator, Orthographic)
- Zoom and pan interactions
- Country selection and hover states
- Economic data visualization with color coding
- Responsive design

**Technical Implementation**:
```typescript
interface InteractiveWorldMapProps {
  data: CountryData[];
  selectedIndicator: string;
  timeRange: { start: Date; end: Date };
  onCountryClick: (country: CountryData) => void;
  onCountryHover: (country: CountryData | null) => void;
  width?: number;
  height?: number;
  className?: string;
}
```

**D3.js Integration**:
- Uses `d3-geo` for map projections
- Implements `d3-zoom` for pan/zoom behavior
- Applies `d3-scale` for color mapping
- Loads TopoJSON data from CDN

#### 2. WorldMapControls

**Purpose**: Control panel for map interactions

**Features**:
- Zoom in/out buttons
- Projection type selector
- Color scheme selector
- Map view state management

#### 3. CountryTooltip

**Purpose**: Displays country information on hover

**Features**:
- Dynamic positioning
- Economic indicator values
- Color-coded values
- Responsive design

#### 4. MapLegend

**Purpose**: Color scale legend for data interpretation

**Features**:
- Gradient color bar
- Min/max value display
- Interactive filtering (future)
- Responsive layout

### Custom Hooks

#### 1. useWorldMap

**Purpose**: Encapsulates D3.js map logic

**Responsibilities**:
- Map projection management
- Zoom behavior configuration
- Path generation
- View state updates

```typescript
export const useWorldMap = (
  svgRef: React.RefObject<SVGSVGElement>,
  initialProjectionType: ProjectionType = 'geoNaturalEarth1',
  initialMapViewState: MapViewState = { scale: 150, translation: [480, 250] }
) => {
  // Returns: projection, path, zoomBehavior, mapViewState, setMapViewState, etc.
};
```

#### 2. useCountryData

**Purpose**: Manages country data processing and color scaling

**Responsibilities**:
- Data normalization
- Color scale generation
- Statistical calculations
- Data filtering and sorting

```typescript
export const useCountryData = (
  countries: CountryData[],
  selectedIndicator: string,
  colorScheme: string = 'viridis'
) => {
  // Returns: processedData, colorScale, dataRange, statistics, etc.
};
```

## State Management

### GlobalAnalysisContext

**Purpose**: Centralized state management for global analysis features

**State Structure**:
```typescript
interface GlobalAnalysisState {
  selectedCountries: string[];
  selectedIndicator: EconomicIndicator;
  mapViewState: MapViewState;
  projectionType: ProjectionType;
  colorScheme: ColorScheme;
  hoveredCountry: string | null;
}
```

**Actions**:
```typescript
interface GlobalAnalysisActions {
  selectCountry: (countryId: string) => void;
  deselectCountry: (countryId: string) => void;
  toggleCountry: (countryId: string) => void;
  setSelectedIndicator: (indicator: EconomicIndicator) => void;
  setMapViewState: (viewState: MapViewState) => void;
  setProjectionType: (type: ProjectionType) => void;
  setColorScheme: (scheme: ColorScheme) => void;
  setHoveredCountry: (countryId: string | null) => void;
}
```

**Performance Optimizations**:
- Split state into logical groups
- Use `useCallback` for action functions
- Memoize computed values with `useMemo`
- Prevent unnecessary re-renders

## Data Flow

### 1. Data Loading

```
Sample Data → useCountryData → Processed Data → Color Scale → Map Rendering
```

### 2. User Interactions

```
User Action → Context Update → Component Re-render → D3 Update → Visual Change
```

### 3. State Updates

```
Component Event → Context Action → State Update → Dependent Components Update
```

## D3.js Integration

### Map Projections

**Supported Projections**:
- `geoNaturalEarth1`: Balanced world view
- `geoMercator`: Rectangular projection
- `geoOrthographic`: Globe view

**Implementation**:
```typescript
const getProjection = useCallback(() => {
  switch (projectionType) {
    case 'geoMercator':
      return geoMercator();
    case 'geoOrthographic':
      return geoOrthographic().rotate([mapViewState.rotation?.[0] || 0, mapViewState.rotation?.[1] || 0]);
    case 'geoNaturalEarth1':
    default:
      return geoNaturalEarth1();
  }
}, [projectionType, mapViewState.rotation]);
```

### Color Scaling

**Color Schemes**:
- Viridis: Perceptually uniform
- Blues: Sequential blue scale
- Reds: Sequential red scale

**Implementation**:
```typescript
const colorScale = useMemo(() => {
  const interpolator = COLOR_SCHEMES[colorScheme] || interpolateViridis;
  return scaleSequential(interpolator).domain([dataRange.min, dataRange.max]);
}, [dataRange, colorScheme]);
```

### Zoom and Pan

**Implementation**:
```typescript
const zoomBehavior = useRef(
  zoom<SVGSVGElement, unknown>()
    .scaleExtent([1, 8])
    .on('zoom', (event) => {
      const { transform } = event;
      setMapViewState((prev) => ({
        ...prev,
        scale: transform.k,
        translation: [transform.x, transform.y],
      }));
    })
);
```

## Type System

### Core Types

```typescript
interface CountryData {
  id: string; // ISO 3166-1 alpha-3 code
  name: string;
  gdp: number;
  inflation: number;
  unemployment: number;
  region?: string;
  subregion?: string;
  economicIndicators?: EconomicIndicator[];
}

interface EconomicIndicator {
  name: string;
  value: number;
  unit: string;
  date: Date;
  source: string;
}

interface MapViewState {
  scale: number;
  translation: [number, number];
  rotation?: [number, number, number];
}
```

### Type Safety Features

- Strict TypeScript configuration
- Comprehensive interface definitions
- Generic type parameters for D3 scales
- Union types for projection and color schemes
- Optional properties with proper null checks

## Testing Strategy

### Unit Tests (Jest + React Testing Library)

**Coverage Areas**:
- Component rendering
- Hook functionality
- Context state management
- D3.js integration mocking
- User interactions

**Test Files**:
- `InteractiveWorldMap.test.tsx`
- `WorldMapControls.test.tsx`
- `CountryTooltip.test.tsx`
- `MapLegend.test.tsx`
- `useWorldMap.test.ts`
- `useCountryData.test.ts`
- `GlobalAnalysisContext.test.tsx`

### Integration Tests (Playwright)

**Test Scenarios**:
- Map loading and rendering
- Country selection workflows
- Indicator switching
- Projection changes
- Responsive design
- Error handling

**Test Files**:
- `global-analysis-enhanced.spec.ts`
- `d3-visualization.spec.ts`
- `global-analysis-context.spec.ts`
- `global-analysis-performance.spec.ts`
- `global-analysis-accessibility.spec.ts`

### Test Configuration

**Jest Configuration**:
```javascript
module.exports = {
  transformIgnorePatterns: [
    'node_modules/(?!(d3|d3-.*|topojson-client)/)'
  ],
  setupFilesAfterEnv: ['<rootDir>/src/setupTests.ts'],
  testEnvironment: 'jsdom',
  moduleNameMapping: {
    '^d3$': '<rootDir>/src/__mocks__/d3.js'
  }
};
```

## Performance Considerations

### Optimization Strategies

1. **D3.js Performance**:
   - Use `useRef` for D3 selections
   - Minimize DOM manipulations
   - Implement efficient data binding
   - Use D3 transitions for smooth animations

2. **React Performance**:
   - Memoize expensive calculations
   - Use `useCallback` for event handlers
   - Split context to prevent unnecessary re-renders
   - Implement virtual scrolling for large datasets

3. **Memory Management**:
   - Clean up D3 event listeners
   - Dispose of unused subscriptions
   - Implement proper component unmounting

### Performance Metrics

- **Initial Load**: < 2 seconds
- **Map Rendering**: < 500ms
- **Interaction Response**: < 100ms
- **Memory Usage**: < 50MB for 200+ countries

## Accessibility

### WCAG 2.1 Compliance

**Level AA Standards**:
- Keyboard navigation support
- Screen reader compatibility
- Color contrast ratios (4.5:1 minimum)
- Focus management
- Alternative text for visual elements

**Implementation**:
```typescript
// Keyboard navigation
const handleKeyDown = (event: KeyboardEvent) => {
  if (event.key === 'Enter' || event.key === ' ') {
    toggleCountry(countryId);
  }
};

// ARIA labels
<svg
  role="img"
  aria-label="Interactive world map showing economic data"
  aria-describedby="map-legend"
>
```

### Accessibility Features

- **Keyboard Navigation**: Full keyboard support for all interactions
- **Screen Reader Support**: Proper ARIA labels and descriptions
- **High Contrast**: Support for high contrast mode
- **Focus Indicators**: Clear focus indicators for all interactive elements
- **Alternative Text**: Descriptive text for visual data representations

## Future Enhancements

### Phase 2 Features (Weeks 4-6)

1. **Advanced Multi-Country Dashboard**:
   - Side-by-side country comparisons
   - Statistical analysis tools
   - Export functionality

2. **Enhanced Visualizations**:
   - Time series animations
   - Correlation networks
   - Heat maps

3. **Data Integration**:
   - Real-time data updates
   - Historical data exploration
   - Custom data sources

### Phase 3 Features (Weeks 7-9)

1. **Event Timeline**:
   - Interactive timeline component
   - Event filtering and search
   - Impact visualization

2. **Network Analysis**:
   - Trade relationship networks
   - Economic correlation graphs
   - Centrality calculations

### Technical Improvements

1. **Performance**:
   - WebGL rendering for large datasets
   - Virtual scrolling
   - Progressive loading

2. **Mobile Optimization**:
   - Touch gesture support
   - Responsive breakpoints
   - Mobile-specific interactions

3. **Advanced Features**:
   - 3D globe visualization
   - VR/AR support
   - Collaborative features

## Conclusion

The Global Analysis architecture provides a robust, scalable foundation for interactive economic data visualization. The combination of React's component model, D3.js's powerful visualization capabilities, and TypeScript's type safety creates a maintainable and extensible system that can grow with future requirements.

The modular design allows for easy testing, debugging, and feature additions while maintaining high performance and accessibility standards. The comprehensive test suite ensures reliability across different browsers and devices.
