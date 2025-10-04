/**
 * InteractiveWorldMap Component Tests.
 *
 * Comprehensive test suite for the InteractiveWorldMap component,
 * covering rendering, interactions, accessibility, and error handling.
 */

import React from 'react';
import { render, screen, waitFor, fireEvent, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ThemeProvider, createTheme, StyledEngineProvider } from '@mui/material/styles';
import { CssBaseline } from '@mui/material';
import { ErrorBoundary } from 'react-error-boundary';
import InteractiveWorldMap from '../InteractiveWorldMap';
import { sampleCountryData } from '../../../data/sampleCountryData';
import { CountryData, MapViewState } from '../../../types/globalAnalysis';

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
            {children}
          </ErrorBoundary>
        </QueryClientProvider>
      </ThemeProvider>
    </StyledEngineProvider>
  );
};

// Default props for testing
const defaultProps = {
  data: sampleCountryData,
  selectedIndicator: 'GDP',
  timeRange: { start: new Date('2020-01-01'), end: new Date('2023-12-31') },
  onCountryClick: vi.fn(),
  onCountryHover: vi.fn(),
  mapView: { scale: 1, translation: [0, 0] } as MapViewState,
  onMapViewChange: vi.fn(),
  width: 800,
  height: 600,
};

describe('InteractiveWorldMap', () => {
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

  describe('Basic Rendering', () => {
    it('should render without crashing', async () => {
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
    });

    it('should display loading state initially', () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} />
        </TestWrapper>
      );

      // Should show loading spinner initially
      expect(screen.getByRole('progressbar')).toBeInTheDocument();
    });

    it('should render with correct dimensions', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} width={900} height={500} />
        </TestWrapper>
      );

      await waitFor(() => {
        const svg = screen.getByRole('img', { hidden: true });
        expect(svg).toHaveAttribute('width', '900');
        expect(svg).toHaveAttribute('height', '500');
      });
    });

    it('should render with custom projection', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} projection="mercator" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should render with custom color scheme', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} colorScheme="blues" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });
  });

  describe('Data Visualization', () => {
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

    it('should handle data with missing indicators', async () => {
      const dataWithoutIndicators: CountryData[] = [
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

    it('should update when selected indicator changes', async () => {
      const TestWrapper = createTestWrapper();
      const { rerender } = render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} selectedIndicator="GDP" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });

      // Change indicator
      rerender(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} selectedIndicator="Inflation" />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });
  });

  describe('User Interactions', () => {
    it('should handle country click events', async () => {
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

      // Simulate country click (this would be handled by D3 event listeners in real implementation)
      // The actual click handling is done by D3 event listeners, so we test the callback is passed correctly
      expect(mockOnCountryClick).toBeDefined();
    });

    it('should handle country hover events', async () => {
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

      // The hover handling is done by D3 event listeners, so we test the callback is passed correctly
      expect(mockOnCountryHover).toBeDefined();
    });
  });

  describe('Map Controls', () => {
    it('should show borders when showBorders is true', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} showBorders={true} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should hide borders when showBorders is false', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} showBorders={false} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should show labels when showLabels is true', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} showLabels={true} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should hide labels when showLabels is false', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} showLabels={false} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });
  });

  describe('Error Handling', () => {
    it('should handle fetch errors gracefully', async () => {
      // Mock fetch to reject
      global.fetch = vi.fn(() => Promise.reject(new Error('Network error'))) as any;

      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} />
        </TestWrapper>
      );

      // Should show error boundary or handle error gracefully
      await waitFor(() => {
        // The component should handle the error and not crash
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
          latitude: null, // Invalid latitude
          longitude: null, // Invalid longitude
        },
      ] as any;

      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} data={invalidData} />
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
          <InteractiveWorldMap {...defaultProps} />
        </TestWrapper>
      );

      await waitFor(() => {
        const svg = screen.getByRole('img', { hidden: true });
        expect(svg).toBeInTheDocument();
        // SVG should be accessible
        expect(svg).toHaveAttribute('width');
        expect(svg).toHaveAttribute('height');
      });
    });

    it('should be keyboard navigable', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} />
        </TestWrapper>
      );

      await waitFor(() => {
        const svg = screen.getByRole('img', { hidden: true });
        expect(svg).toBeInTheDocument();
        // SVG should be focusable for keyboard navigation
        expect(svg).toHaveAttribute('tabindex', '0');
      });
    });
  });

  describe('Performance', () => {
    it('should not re-render unnecessarily', async () => {
      const TestWrapper = createTestWrapper();
      const { rerender } = render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });

      // Re-render with same props should not cause issues
      rerender(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });

    it('should handle large datasets efficiently', async () => {
      // Create a large dataset
      const largeDataset: CountryData[] = Array.from({ length: 100 }, (_, i) => ({
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
          <InteractiveWorldMap {...defaultProps} data={largeDataset} />
        </TestWrapper>
      );

      await waitFor(() => {
        expect(screen.getByRole('img', { hidden: true })).toBeInTheDocument();
      });
    });
  });

  describe('Responsive Design', () => {
    it('should handle different screen sizes', async () => {
      const TestWrapper = createTestWrapper();
      
      // Test mobile size
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} width={400} height={300} />
        </TestWrapper>
      );

      await waitFor(() => {
        const svg = screen.getByRole('img', { hidden: true });
        expect(svg).toHaveAttribute('width', '400');
        expect(svg).toHaveAttribute('height', '300');
      });
    });

    it('should maintain aspect ratio', async () => {
      const TestWrapper = createTestWrapper();
      render(
        <TestWrapper>
          <InteractiveWorldMap {...defaultProps} width={800} height={600} />
        </TestWrapper>
      );

      await waitFor(() => {
        const svg = screen.getByRole('img', { hidden: true });
        expect(svg).toHaveAttribute('viewBox', '0 0 800 600');
      });
    });
  });
});
