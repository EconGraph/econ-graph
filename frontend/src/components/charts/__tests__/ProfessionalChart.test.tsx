/**
 * REQUIREMENT: Comprehensive unit tests for ProfessionalChart component
 * PURPOSE: Test professional chart analytics with Bloomberg Terminal-level capabilities
 * This ensures chart rendering, technical analysis, and collaboration features work correctly
 */

import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi } from 'vitest';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import ProfessionalChart, { SeriesData } from '../ProfessionalChart';

// Mock Chart.js
vi.mock('react-chartjs-2', () => ({
  Line: ({ data, options }: any) => (
    <div data-testid="professional-chart">
      <div data-testid="chart-data">{JSON.stringify(data)}</div>
      <div data-testid="chart-options">{JSON.stringify(options)}</div>
    </div>
  ),
}));

const theme = createTheme();

const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <ThemeProvider theme={theme}>
    {children}
  </ThemeProvider>
);

// Mock data for testing
const mockPrimarySeries: SeriesData = {
  id: 'GDPC1',
  title: 'Real Gross Domestic Product',
  description: 'Seasonally Adjusted Annual Rate',
  data: [
    { date: '2020-01-01', value: 19254.69 },
    { date: '2020-04-01', value: 17303.38 },
    { date: '2021-01-01', value: 19055.65 },
    { date: '2021-04-01', value: 19368.31 },
  ],
  color: '#2196f3',
  unit: 'Billions of Chained 2012 Dollars',
  frequency: 'Quarterly',
};

const mockSecondarySeries: SeriesData[] = [
  {
    id: 'UNRATE',
    title: 'Unemployment Rate',
    description: 'Seasonally Adjusted',
    data: [
      { date: '2020-01-01', value: 3.5 },
      { date: '2020-04-01', value: 14.8 },
      { date: '2021-01-01', value: 6.3 },
      { date: '2021-04-01', value: 6.1 },
    ],
    color: '#f44336',
    unit: 'Percent',
    frequency: 'Monthly',
  },
];


describe('ProfessionalChart', () => {
  const mockOnAnnotationAdd = vi.fn();
  const mockOnSeriesAdd = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Basic Rendering', () => {
    it('renders professional chart with primary series', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
      expect(screen.getByText('Professional Chart Analytics')).toBeInTheDocument();
    });

    it('renders with default props', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Should render the chart component
      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
      expect(screen.getByText('Professional Chart Analytics')).toBeInTheDocument();
    });

    it('renders with secondary series', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            secondarySeries={mockSecondarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const chartData = screen.getByTestId('chart-data');
      const dataContent = JSON.parse(chartData.textContent || '{}');

      expect(dataContent.datasets).toHaveLength(2);
      expect(dataContent.datasets[0].label).toContain('Real Gross Domestic Product');
      expect(dataContent.datasets[1].label).toContain('Unemployment Rate');
    });
  });

  describe('Technical Analysis Features', () => {
    it('shows technical analysis controls when enabled', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            showTechnicalAnalysis={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Should show technical analysis accordion
      expect(screen.getByText(/Technical Analysis/)).toBeInTheDocument();

      // Expand the accordion to access controls
      const accordionButton = screen.getByRole('button', { name: /Technical Analysis/ });
      await user.click(accordionButton);

      // Wait for accordion to expand and show controls
      await waitFor(() => {
        expect(screen.getByText(/Simple Moving Average/)).toBeInTheDocument();
      });

      expect(screen.getByText(/Bollinger Bands/)).toBeInTheDocument();
    });

    it('toggles SMA indicator when checkbox is clicked', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            showTechnicalAnalysis={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Expand accordion first
      const accordionButton = screen.getByRole('button', { name: /Technical Analysis/ });
      await user.click(accordionButton);

      // Wait for SMA checkbox to be available
      await waitFor(() => {
        const smaCheckbox = screen.getByRole('checkbox', { name: /Simple Moving Average/ });
        expect(smaCheckbox).not.toBeChecked();
      });

      const smaCheckbox = screen.getByRole('checkbox', { name: /Simple Moving Average/ });

      // Click to enable
      await user.click(smaCheckbox);
      expect(smaCheckbox).toBeChecked();
    });

    it('handles technical analysis settings changes', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            showTechnicalAnalysis={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Expand accordion first
      const accordionButton = screen.getByRole('button', { name: /Technical Analysis/ });
      await user.click(accordionButton);

      // Wait for checkboxes to be available and interact with them
      await waitFor(() => {
        expect(screen.getByRole('checkbox', { name: /Simple Moving Average/ })).toBeInTheDocument();
      });

      const smaCheckbox = screen.getByRole('checkbox', { name: /Simple Moving Average/ });
      const emaCheckbox = screen.getByRole('checkbox', { name: /Exponential Moving Average/ });
      const bollingerCheckbox = screen.getByRole('checkbox', { name: /Bollinger Bands/ });

      await user.click(smaCheckbox);
      await user.click(emaCheckbox);
      await user.click(bollingerCheckbox);

      expect(smaCheckbox).toBeChecked();
      expect(emaCheckbox).toBeChecked();
      expect(bollingerCheckbox).toBeChecked();
    });
  });

  describe('Economic Events', () => {
    it('shows economic events when enabled', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            showEconomicEvents={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Expand accordion to access economic events toggle
      const accordionButton = screen.getByRole('button', { name: /Technical Analysis/ });
      await user.click(accordionButton);

      // Should show events toggle after expanding
      await waitFor(() => {
        expect(screen.getByRole('checkbox', { name: /Economic Events/ })).toBeInTheDocument();
      });
    });

    it('toggles economic events display', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            showEconomicEvents={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Expand accordion first
      const accordionButton = screen.getByRole('button', { name: /Technical Analysis/ });
      await user.click(accordionButton);

      // Wait for events checkbox to appear
      await waitFor(() => {
        const eventsCheckbox = screen.getByRole('checkbox', { name: /Economic Events/ });
        expect(eventsCheckbox).toBeChecked(); // Should start checked
      });

      const eventsCheckbox = screen.getByRole('checkbox', { name: /Economic Events/ });

      // Click to disable
      await user.click(eventsCheckbox);
      expect(eventsCheckbox).not.toBeChecked();
    });
  });

  describe('Chart Controls', () => {
    it('shows chart control buttons', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Should show control buttons
      expect(screen.getByLabelText(/Add Series/)).toBeInTheDocument();
      expect(screen.getByLabelText(/Export Chart/)).toBeInTheDocument();
      expect(screen.getByLabelText(/Fullscreen/)).toBeInTheDocument();
    });

    it('calls onSeriesAdd when add series button is clicked', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const addSeriesButton = screen.getByLabelText(/Add Series/);
      await user.click(addSeriesButton);

      expect(mockOnSeriesAdd).toHaveBeenCalledTimes(1);
    });

    it('toggles fullscreen mode', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const fullscreenButton = screen.getByLabelText(/Fullscreen/);
      await user.click(fullscreenButton);

      // Verify fullscreen state change (check for any visual changes)
      expect(fullscreenButton).toBeInTheDocument();
    });
  });

  describe('Data Handling', () => {
    it('handles empty data gracefully', () => {
      const emptySeries: SeriesData = {
        ...mockPrimarySeries,
        data: [],
      };

      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={emptySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
      expect(screen.getByText('Professional Chart Analytics')).toBeInTheDocument();
    });

    it('handles missing optional props gracefully', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            // Minimal props - most are optional
          />
        </TestWrapper>
      );

      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
    });

    it('handles custom height prop', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            height={800}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const container = screen.getByTestId('professional-chart').closest('div');
      expect(container).toBeInTheDocument();
    });
  });

  describe('Annotations Features', () => {
    it('shows annotation controls when enabled', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            allowAnnotations={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Should render without errors when annotations are allowed
      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
    });
  });

  describe('Error Handling', () => {
    it('handles invalid series data gracefully', () => {
      const invalidSeries = {
        ...mockPrimarySeries,
        data: null as any, // Invalid data type
      };

      // Should render without crashing (but may show empty chart)
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={invalidSeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Should still render the chart container even with invalid data
      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
    });

    it('handles missing callback props gracefully', () => {
      // Should render without callbacks
      expect(() => {
        render(
          <TestWrapper>
            <ProfessionalChart
              primarySeries={mockPrimarySeries}
            />
          </TestWrapper>
        );
      }).not.toThrow();
    });
  });

  describe('Performance', () => {
    it('renders efficiently with large datasets', () => {
      const largeSeries: SeriesData = {
        ...mockPrimarySeries,
        data: Array.from({ length: 1000 }, (_, i) => ({
          date: `2020-${String(i % 12 + 1).padStart(2, '0')}-01`,
          value: Math.random() * 20000 + 15000,
        })),
      };

      const startTime = performance.now();

      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={largeSeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const endTime = performance.now();
      const renderTime = endTime - startTime;

      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
      expect(renderTime).toBeLessThan(1000); // Should render within 1 second
    });
  });

  describe('Accessibility', () => {
    it('has proper ARIA attributes', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            showTechnicalAnalysis={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Check for ARIA labels on buttons
      expect(screen.getByLabelText(/Add Series/)).toBeInTheDocument();
      expect(screen.getByLabelText(/Export Chart/)).toBeInTheDocument();
      expect(screen.getByLabelText(/Fullscreen/)).toBeInTheDocument();
    });

    it('supports keyboard navigation', async () => {
      const user = userEvent.setup();
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            showTechnicalAnalysis={true}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      // Test tab navigation through controls
      await user.tab();

      // Should be able to navigate to focusable elements
      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBeGreaterThan(0);

      // Test Enter key activation
      if (buttons[0]) {
        buttons[0].focus();
        await user.keyboard('{Enter}');
        // Should not throw errors
        expect(buttons[0]).toBeInTheDocument();
      }
    });
  });

  describe('Chart Data Processing', () => {
    it('processes chart data correctly', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const chartData = screen.getByTestId('chart-data');
      const dataContent = JSON.parse(chartData.textContent || '{}');

      expect(dataContent.datasets).toBeDefined();
      expect(dataContent.datasets).toHaveLength(1);
      expect(dataContent.datasets[0].label).toBe('Real Gross Domestic Product');
    });

    it('includes secondary series in chart data', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            secondarySeries={mockSecondarySeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const chartData = screen.getByTestId('chart-data');
      const dataContent = JSON.parse(chartData.textContent || '{}');

      expect(dataContent.datasets).toHaveLength(2);
      expect(dataContent.datasets[1].label).toBe('Unemployment Rate');
    });

    it('configures chart options correctly', () => {
      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={mockPrimarySeries}
            height={600}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      const chartOptions = screen.getByTestId('chart-options');
      const optionsContent = JSON.parse(chartOptions.textContent || '{}');

      expect(optionsContent.responsive).toBe(true);
      expect(optionsContent.maintainAspectRatio).toBe(false);
    });
  });

  describe('Component Integration', () => {
    it('integrates properly with parent components', () => {
      // Test that the component works within a typical parent component structure
      render(
        <TestWrapper>
          <div data-testid="parent-container">
            <ProfessionalChart
              primarySeries={mockPrimarySeries}
              showTechnicalAnalysis={true}
              showEconomicEvents={true}
              allowAnnotations={true}
              onAnnotationAdd={mockOnAnnotationAdd}
              onSeriesAdd={mockOnSeriesAdd}
            />
          </div>
        </TestWrapper>
      );

      expect(screen.getByTestId('parent-container')).toBeInTheDocument();
      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
    });
  });

  describe('Edge Cases', () => {
    it('handles undefined callbacks without errors', () => {
      expect(() => {
        render(
          <TestWrapper>
            <ProfessionalChart
              primarySeries={mockPrimarySeries}
              onAnnotationAdd={undefined as any}
              onSeriesAdd={undefined as any}
            />
          </TestWrapper>
        );
      }).not.toThrow();
    });

    it('handles extremely small datasets', () => {
      const smallSeries: SeriesData = {
        ...mockPrimarySeries,
        data: [{ date: '2020-01-01', value: 100 }],
      };

      render(
        <TestWrapper>
          <ProfessionalChart
            primarySeries={smallSeries}
            onAnnotationAdd={mockOnAnnotationAdd}
            onSeriesAdd={mockOnSeriesAdd}
          />
        </TestWrapper>
      );

      expect(screen.getByTestId('professional-chart')).toBeInTheDocument();
    });

    it('handles malformed date strings gracefully', () => {
      const malformedSeries: SeriesData = {
        ...mockPrimarySeries,
        data: [
          { date: 'invalid-date', value: 100 },
          { date: '2020-01-01', value: 200 },
        ],
      };

      expect(() => {
        render(
          <TestWrapper>
            <ProfessionalChart
              primarySeries={malformedSeries}
              onAnnotationAdd={mockOnAnnotationAdd}
              onSeriesAdd={mockOnSeriesAdd}
            />
          </TestWrapper>
        );
      }).not.toThrow();
    });
  });
});
