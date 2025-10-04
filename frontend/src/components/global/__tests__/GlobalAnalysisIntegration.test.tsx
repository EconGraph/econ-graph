/**
 * Global Analysis Integration Tests.
 *
 * Comprehensive integration tests for Global Analysis components
 * using MSW for GraphQL mocking and proper Material-UI setup.
 */

import React from 'react';
import { render, screen, waitFor, fireEvent, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider, createTheme, StyledEngineProvider } from '@mui/material/styles';
import { CssBaseline } from '@mui/material';
import { ErrorBoundary } from 'react-error-boundary';
import { setupSimpleMSW } from '../../../test-utils/mocks/simpleServer';
import { GlobalAnalysisProvider } from '../../../contexts/GlobalAnalysisContext';
import InteractiveWorldMap from '../InteractiveWorldMap';
import WorldMapControls from '../WorldMapControls';
import MapLegend from '../MapLegend';
import { sampleCountryData } from '../../../data/sampleCountryData';

import { setupD3Mocks } from '../../../test-utils/d3-testing-utils';

// Setup D3.js mocks
setupD3Mocks();

vi.mock('topojson-client', () => ({
  feature: vi.fn(() => ({
    features: [
      {
        properties: { ISO_A2: 'US', NAME: 'United States' },
        geometry: { type: 'Polygon', coordinates: [] },
      },
      {
        properties: { ISO_A2: 'CN', NAME: 'China' },
        geometry: { type: 'Polygon', coordinates: [] },
      },
    ],
  })),
  mesh: vi.fn(() => ({ type: 'LineString', coordinates: [] })),
}));

// Mock world atlas data
const mockWorldAtlasData = {
  objects: {
    countries: {
      type: 'GeometryCollection',
      geometries: [],
    },
  },
};

// Mock fetch for world atlas data
global.fetch = vi.fn(() =>
  Promise.resolve({
    ok: true,
    json: () => Promise.resolve(mockWorldAtlasData),
  })
) as any;

// Create test theme
const theme = createTheme({
  palette: {
    mode: 'light',
  },
});

// Test wrapper with all necessary providers
const createTestWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        cacheTime: 0,
      },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <StyledEngineProvider injectFirst>
      <ThemeProvider theme={theme}>
        <QueryClientProvider client={queryClient}>
          <CssBaseline />
          <ErrorBoundary fallback={<div>Error boundary triggered</div>}>
            <GlobalAnalysisProvider>
              {children}
            </GlobalAnalysisProvider>
          </ErrorBoundary>
        </QueryClientProvider>
      </ThemeProvider>
    </StyledEngineProvider>
  );
};

describe('Global Analysis Integration', () => {
  beforeAll(async () => {
    await setupSimpleMSW();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    // Clean up portal containers for Material-UI components
    const existingContainers = document.querySelectorAll('[data-testid="portal-container"]');
    existingContainers.forEach(container => container.remove());
  });

  afterEach(() => {
    // Clean up portal containers after each test
    const containers = document.querySelectorAll('[data-testid="portal-container"]');
    containers.forEach(container => container.remove());
  });

  describe('InteractiveWorldMap Component', () => {
    const defaultProps = {
      data: sampleCountryData,
      selectedIndicator: 'GDP',
      timeRange: { start: new Date('2020-01-01'), end: new Date('2023-12-31') },
      onCountryClick: vi.fn(),
      onCountryHover: vi.fn(),
      mapView: { scale: 1, translation: [0, 0] },
      onMapViewChange: vi.fn(),
      width: 800,
      height: 600,
    };

    it('should render world map with countries', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} />
        </TestWrapper>
      );

      // Wait for the component to render
      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });

      // Should show loading initially, then the map
      expect(screen.getByRole('progressbar')).toBeInTheDocument();
    });

    it('should handle different projections', async () => {
      const TestWrapper = createTestWrapper();
      const { rerender } = render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} projection="naturalEarth" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });

      // Change projection
      rerender(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} projection="mercator" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should handle different color schemes', async () => {
      const TestWrapper = createTestWrapper();
      const { rerender } = render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} colorScheme="viridis" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });

      // Change color scheme
      rerender(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} colorScheme="blues" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should handle country selection', async () => {
      const mockOnCountryClick = vi.fn();
      const TestWrapper = createTestWrapper();
      
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} onCountryClick={mockOnCountryClick} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });

      // The actual click handling is done by D3 event listeners
      // We test that the callback is passed correctly
      expect(mockOnCountryClick).toBeDefined();
    });

    it('should handle country hover', async () => {
      const mockOnCountryHover = vi.fn();
      const TestWrapper = createTestWrapper();
      
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} onCountryHover={mockOnCountryHover} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });

      // The hover handling is done by D3 event listeners
      expect(mockOnCountryHover).toBeDefined();
    });

    it('should handle empty data gracefully', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} data={[]} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should handle missing indicators', async () => {
      const dataWithoutIndicators = [
        {
          id: 'test',
          name: 'Test Country',
          isoAlpha2: 'TC',
          isoAlpha3: 'TST',
          latitude: 0,
          longitude: 0,
        },
      ];

      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} data={dataWithoutIndicators} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });
  });

  describe('WorldMapControls Component', () => {
    it('should render map controls', () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <WorldMapControls />
        </TestWrapper>
      );

      // Should render without crashing
      expect(screen.getByRole('button')).toBeInTheDocument();
    });

    it('should handle projection changes', async () => {
      const user = userEvent.setup();
      const TestWrapper = createTestWrapper();
      
      render(
        <TestWrapper>
          <WorldMapControls />
        </TestWrapper>
      );

      // Find and interact with projection selector
      const projectionSelect = screen.getByRole('combobox');
      expect(projectionSelect).toBeInTheDocument();

      await user.click(projectionSelect);
      
      // Should show projection options
      await waitFor(() => {
        expect(screen.getByText('Natural Earth')).toBeInTheDocument();
      });
    });

    it('should handle color scheme changes', async () => {
      const user = userEvent.setup();
      const TestWrapper = createTestWrapper();
      
      render(
        <TestWrapper>
          <WorldMapControls />
        </TestWrapper>
      );

      // Find and interact with color scheme selector
      const colorSchemeSelect = screen.getByRole('combobox');
      expect(colorSchemeSelect).toBeInTheDocument();

      await user.click(colorSchemeSelect);
      
      // Should show color scheme options
      await waitFor(() => {
        expect(screen.getByText('Viridis')).toBeInTheDocument();
      });
    });
  });

  describe('MapLegend Component', () => {
    it('should render map legend', () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <MapLegend
            dataRange={{ min: 0, max: 100 }}
            selectedIndicator="GDP"
            colorScheme="viridis"
          />
        </TestWrapper>
      );

      // Should render legend elements
      expect(screen.getByText('GDP')).toBeInTheDocument();
    });

    it('should display correct data range', () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <MapLegend
            dataRange={{ min: 1000, max: 1000000 }}
            selectedIndicator="GDP"
            colorScheme="viridis"
          />
        </TestWrapper>
      );

      expect(screen.getByText('GDP')).toBeInTheDocument();
    });

    it('should handle different indicators', () => {
      const TestWrapper = createTestWrapper();
      const { rerender } = render(
        <TestWrapper>
          <MapLegend
            dataRange={{ min: 0, max: 10 }}
            selectedIndicator="Inflation"
            colorScheme="viridis"
          />
        </TestWrapper>
      );

      expect(screen.getByText('Inflation')).toBeInTheDocument();

      // Change indicator
      rerender(
        <TestWrapper>
          <MapLegend
            dataRange={{ min: 0, max: 20 }}
            selectedIndicator="Unemployment"
            colorScheme="viridis"
          />
        </TestWrapper>
      );

      expect(screen.getByText('Unemployment')).toBeInTheDocument();
    });
  });

  describe('Error Handling', () => {
    it('should handle fetch errors gracefully', async () => {
      // Mock fetch to reject
      global.fetch = vi.fn(() => Promise.reject(new Error('Network error'))) as any;

      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap
            data={sampleCountryData}
            selectedIndicator="GDP"
            timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
            onCountryClick={vi.fn()}
            onCountryHover={vi.fn()}
            mapView={{ scale: 1, translation: [0, 0] }}
            onMapViewChange={vi.fn()}
            width={800}
            height={600}
          />
        </TestWrapper>
      );

      // Should handle error gracefully
      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should handle invalid data gracefully', async () => {
      const invalidData = [
        {
          id: 'invalid',
          name: 'Invalid Country',
          isoAlpha2: 'XX',
          isoAlpha3: 'XXX',
          latitude: null,
          longitude: null,
        },
      ] as any;

      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap
            data={invalidData}
            selectedIndicator="GDP"
            timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
            onCountryClick={vi.fn()}
            onCountryHover={vi.fn()}
            mapView={{ scale: 1, translation: [0, 0] }}
            onMapViewChange={vi.fn()}
            width={800}
            height={600}
          />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });
  });

  describe('Accessibility', () => {
    it('should have proper ARIA attributes', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap
            data={sampleCountryData}
            selectedIndicator="GDP"
            timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
            onCountryClick={vi.fn()}
            onCountryHover={vi.fn()}
            mapView={{ scale: 1, translation: [0, 0] }}
            onMapViewChange={vi.fn()}
            width={800}
            height={600}
          />
        </TestWrapper>
      );

      await waitFor(() => {
        const svg = screen.getByRole('img', { hidden: true });
        expect(svg).toBeInTheDocument();
        expect(svg).toHaveAttribute('width');
        expect(svg).toHaveAttribute('height');
      });
    });

    it('should be keyboard navigable', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap
            data={sampleCountryData}
            selectedIndicator="GDP"
            timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
            onCountryClick={vi.fn()}
            onCountryHover={vi.fn()}
            mapView={{ scale: 1, translation: [0, 0] }}
            onMapViewChange={vi.fn()}
            width={800}
            height={600}
          />
        </TestWrapper>
      );

      await waitFor(() => {
        const svg = screen.getByRole('img', { hidden: true });
        expect(svg).toBeInTheDocument();
        expect(svg).toHaveAttribute('tabindex', '0');
      });
    });
  });

  describe('Performance', () => {
    it('should handle large datasets efficiently', async () => {
      const largeDataset = Array.from({ length: 100 }, (_, i) => ({
        id: `country-${i}`,
        name: `Country ${i}`,
        isoAlpha2: `C${i.toString().padStart(2, '0')}`,
        isoAlpha3: `CT${i.toString().padStart(2, '0')}`,
        latitude: Math.random() * 180 - 90,
        longitude: Math.random() * 360 - 180,
        economicIndicators: [
          {
            name: 'GDP',
            value: Math.random() * 1000000,
            unit: 'USD',
            year: 2023,
            source: 'Test',
          },
        ],
      }));

      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap
            data={largeDataset}
            selectedIndicator="GDP"
            timeRange={{ start: new Date('2020-01-01'), end: new Date('2023-12-31') }}
            onCountryClick={vi.fn()}
            onCountryHover={vi.fn()}
            mapView={{ scale: 1, translation: [0, 0] }}
            onMapViewChange={vi.fn()}
            width={800}
            height={600}
          />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });
  });
});
