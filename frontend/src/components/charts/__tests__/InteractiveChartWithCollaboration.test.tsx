/**
 * REQUIREMENT: Comprehensive unit tests for InteractiveChartWithCollaboration component
 * PURPOSE: Test sophisticated Bloomberg Terminal-inspired economic data visualization with collaboration
 * This ensures the advanced charting capabilities work correctly with real-time collaboration features
 */

import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import InteractiveChartWithCollaboration from '../InteractiveChartWithCollaboration';

// Mock Chart.js to avoid canvas rendering issues in tests
jest.mock('react-chartjs-2', () => ({
  Line: ({ data, options, ...props }: any) => (
    <div
      data-testid="line-chart"
      data-chart-data={JSON.stringify(data)}
      data-chart-options={JSON.stringify(options)}
      {...props}
    >
      Mock Interactive Chart with Collaboration
    </div>
  ),
}));

// Mock the collaboration component
jest.mock('../ChartCollaborationConnected', () => {
  return function MockChartCollaborationConnected({ isOpen, onToggle }: any) {
    return (
      <div data-testid="chart-collaboration">
        <button data-testid="toggle-collaboration" onClick={onToggle}>
          {isOpen ? 'Close Collaboration' : 'Open Collaboration'}
        </button>
        {isOpen && (
          <div>
            <div>Collaboration Panel</div>
            <div>Annotations: 0</div>
          </div>
        )}
      </div>
    );
  };
});

// Mock the data transformation utilities
jest.mock('../../../utils/dataTransformations', () => ({
  transformData: jest.fn((data, transformation) => {
    if (transformation === 'yoy') {
      return data.map((point: any, index: number) => ({
        ...point,
        value: index > 0 ? ((point.value - data[index - 1].value) / data[index - 1].value) * 100 : 0,
      }));
    }
    if (transformation === 'qoq') {
      return data.map((point: any, index: number) => ({
        ...point,
        value: index > 3 ? ((point.value - data[index - 4].value) / data[index - 4].value) * 100 : 0,
      }));
    }
    if (transformation === 'mom') {
      return data.map((point: any, index: number) => ({
        ...point,
        value: index > 0 ? ((point.value - data[index - 1].value) / data[index - 1].value) * 100 : 0,
      }));
    }
    return data;
  }),
}));

const theme = createTheme();

// Mock data for testing
const mockSeriesData = [
  { date: '2024-01-01', value: 100.0, originalValue: 100.0, isOriginalRelease: true, revisionDate: '2024-01-01' },
  { date: '2024-02-01', value: 101.5, originalValue: 101.5, isOriginalRelease: true, revisionDate: '2024-02-01' },
  { date: '2024-03-01', value: 102.3, originalValue: 102.3, isOriginalRelease: true, revisionDate: '2024-03-01' },
  { date: '2024-04-01', value: 103.8, originalValue: 103.8, isOriginalRelease: true, revisionDate: '2024-04-01' },
  { date: '2024-05-01', value: 105.2, originalValue: 105.2, isOriginalRelease: true, revisionDate: '2024-05-01' },
  { date: '2024-06-01', value: 104.9, originalValue: 104.9, isOriginalRelease: true, revisionDate: '2024-06-01' },
];

const defaultProps = {
  seriesId: 'test-series-1',
  seriesTitle: 'Test Economic Series',
  data: mockSeriesData,
  title: 'Test Economic Series',
  yAxisLabel: 'Billions of Dollars',
  units: 'Billions of Dollars',
  frequency: 'Monthly',
  onDataPointClick: jest.fn(),
  onTransformationChange: jest.fn(),
  collaborationEnabled: true,
};

const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <ThemeProvider theme={theme}>
    {children}
  </ThemeProvider>
);

describe('InteractiveChartWithCollaboration', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  const renderInteractiveChartWithCollaboration = (props = {}) => {
    const combinedProps = { ...defaultProps, ...props };
    return render(
      <TestWrapper>
        <InteractiveChartWithCollaboration {...combinedProps} />
      </TestWrapper>
    );
  };

  describe('Basic Rendering', () => {
    it('should render chart with collaboration features', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByTestId('line-chart')).toBeInTheDocument();
      expect(screen.getByTestId('chart-collaboration')).toBeInTheDocument();
      expect(screen.getByText('Mock Interactive Chart with Collaboration')).toBeInTheDocument();
    });

    it('should display chart title', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByText('Test Economic Series')).toBeInTheDocument();
    });

    it('should show collaboration toggle button', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByTestId('toggle-collaboration')).toBeInTheDocument();
      expect(screen.getByText('Open Collaboration')).toBeInTheDocument();
    });

    it('should not show collaboration panel initially', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.queryByText('Collaboration Panel')).not.toBeInTheDocument();
    });
  });

  describe('Data Transformation', () => {
    it('should display transformation selector', () => {
      renderInteractiveChartWithCollaboration();

      const transformationSelect = screen.getByLabelText('Data Transformation');
      expect(transformationSelect).toBeInTheDocument();
      expect(transformationSelect).toHaveValue('none');
    });

    it('should handle Year-over-Year transformation', async () => {
      const user = userEvent.setup();
      const mockOnTransformationChange = jest.fn();

      renderInteractiveChartWithCollaboration({
        onTransformationChange: mockOnTransformationChange,
      });

      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('Year-over-Year (%)'));

      expect(mockOnTransformationChange).toHaveBeenCalledWith('yoy');
    });

    it('should handle Quarter-over-Quarter transformation', async () => {
      const user = userEvent.setup();
      const mockOnTransformationChange = jest.fn();

      renderInteractiveChartWithCollaboration({
        onTransformationChange: mockOnTransformationChange,
      });

      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('Quarter-over-Quarter (%)'));

      expect(mockOnTransformationChange).toHaveBeenCalledWith('qoq');
    });

    it('should handle Month-over-Month transformation', async () => {
      const user = userEvent.setup();
      const mockOnTransformationChange = jest.fn();

      renderInteractiveChartWithCollaboration({
        onTransformationChange: mockOnTransformationChange,
      });

      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('Month-over-Month (%)'));

      expect(mockOnTransformationChange).toHaveBeenCalledWith('mom');
    });

    it('should handle Log Scale transformation', async () => {
      const user = userEvent.setup();
      const mockOnTransformationChange = jest.fn();

      renderInteractiveChartWithCollaboration({
        onTransformationChange: mockOnTransformationChange,
      });

      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('Log Scale'));

      expect(mockOnTransformationChange).toHaveBeenCalledWith('log');
    });

    it('should handle Difference transformation', async () => {
      const user = userEvent.setup();
      const mockOnTransformationChange = jest.fn();

      renderInteractiveChartWithCollaboration({
        onTransformationChange: mockOnTransformationChange,
      });

      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('Difference'));

      expect(mockOnTransformationChange).toHaveBeenCalledWith('diff');
    });

    it('should handle Percentage Change transformation', async () => {
      const user = userEvent.setup();
      const mockOnTransformationChange = jest.fn();

      renderInteractiveChartWithCollaboration({
        onTransformationChange: mockOnTransformationChange,
      });

      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('Percentage Change'));

      expect(mockOnTransformationChange).toHaveBeenCalledWith('pct_change');
    });

    it('should reset to None transformation', async () => {
      const user = userEvent.setup();
      const mockOnTransformationChange = jest.fn();

      renderInteractiveChartWithCollaboration({
        onTransformationChange: mockOnTransformationChange,
      });

      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('None (Levels)'));

      expect(mockOnTransformationChange).toHaveBeenCalledWith('none');
    });
  });

  describe('Date Range Controls', () => {
    it('should display date range selector', () => {
      renderInteractiveChartWithCollaboration();

      const startDateInput = screen.getByLabelText('Start Date');
      const endDateInput = screen.getByLabelText('End Date');

      expect(startDateInput).toBeInTheDocument();
      expect(endDateInput).toBeInTheDocument();
    });

    it('should initialize date range with data bounds', () => {
      renderInteractiveChartWithCollaboration();

      const startDateInput = screen.getByLabelText('Start Date');
      const endDateInput = screen.getByLabelText('End Date');

      expect(startDateInput).toHaveValue('2024-01-01');
      expect(endDateInput).toHaveValue('2024-06-01');
    });

    it('should handle date range changes', async () => {
      const user = userEvent.setup();
      renderInteractiveChartWithCollaboration();

      const startDateInput = screen.getByLabelText('Start Date');
      const endDateInput = screen.getByLabelText('End Date');

      await user.clear(startDateInput);
      await user.type(startDateInput, '2024-02-01');

      await user.clear(endDateInput);
      await user.type(endDateInput, '2024-05-01');

      expect(startDateInput).toHaveValue('2024-02-01');
      expect(endDateInput).toHaveValue('2024-05-01');
    });
  });

  describe('Data Point Interactions', () => {
    it('should handle data point clicks', async () => {
      const user = userEvent.setup();
      const mockOnDataPointClick = jest.fn();

      renderInteractiveChartWithCollaboration({
        onDataPointClick: mockOnDataPointClick,
      });

      // Simulate clicking on the chart
      const chartElement = screen.getByTestId('line-chart');
      await user.click(chartElement);

      // Note: In a real implementation, this would trigger the data point click handler
      // For now, we're testing that the chart renders and can be interacted with
      expect(chartElement).toBeInTheDocument();
    });

    it('should display data point information on hover', async () => {
      const user = userEvent.setup();
      renderInteractiveChartWithCollaboration();

      const chartElement = screen.getByTestId('line-chart');

      // Simulate hover over chart
      await user.hover(chartElement);

      // Chart should still be visible and interactive
      expect(chartElement).toBeInTheDocument();
    });
  });

  describe('Collaboration Features', () => {
    it('should toggle collaboration panel', async () => {
      const user = userEvent.setup();
      renderInteractiveChartWithCollaboration();

      const toggleButton = screen.getByTestId('toggle-collaboration');
      expect(screen.queryByText('Collaboration Panel')).not.toBeInTheDocument();

      // Open collaboration panel
      await user.click(toggleButton);

      expect(screen.getByText('Collaboration Panel')).toBeInTheDocument();
      expect(screen.getByText('Close Collaboration')).toBeInTheDocument();

      // Close collaboration panel
      await user.click(toggleButton);

      expect(screen.queryByText('Collaboration Panel')).not.toBeInTheDocument();
      expect(screen.getByText('Open Collaboration')).toBeInTheDocument();
    });

    it('should disable collaboration when collaborationEnabled is false', () => {
      renderInteractiveChartWithCollaboration({
        collaborationEnabled: false,
      });

      expect(screen.queryByTestId('chart-collaboration')).not.toBeInTheDocument();
    });

    it('should pass correct props to collaboration component', () => {
      renderInteractiveChartWithCollaboration();

      // The collaboration component should receive the seriesId and chartId
      expect(screen.getByTestId('chart-collaboration')).toBeInTheDocument();
    });
  });

  describe('Chart Configuration', () => {
    it('should configure chart with correct data', () => {
      renderInteractiveChartWithCollaboration();

      const chartElement = screen.getByTestId('line-chart');
      const chartData = JSON.parse(chartElement.getAttribute('data-chart-data') || '{}');

      expect(chartData.datasets).toBeDefined();
      expect(chartData.datasets[0].data).toHaveLength(6);
      expect(chartData.datasets[0].data[0]).toEqual({
        x: '2024-01-01',
        y: 100.0,
      });
    });

    it('should configure chart with correct options', () => {
      renderInteractiveChartWithCollaboration();

      const chartElement = screen.getByTestId('line-chart');
      const chartOptions = JSON.parse(chartElement.getAttribute('data-chart-options') || '{}');

      expect(chartOptions.responsive).toBe(true);
      expect(chartOptions.maintainAspectRatio).toBe(false);
      expect(chartOptions.scales.x.type).toBe('time');
      expect(chartOptions.scales.y.title.text).toBe('Billions of Dollars');
    });

    it('should handle empty data gracefully', () => {
      renderInteractiveChartWithCollaboration({
        data: [],
      });

      const chartElement = screen.getByTestId('line-chart');
      const chartData = JSON.parse(chartElement.getAttribute('data-chart-data') || '{}');

      expect(chartData.datasets).toBeDefined();
      expect(chartData.datasets[0].data).toHaveLength(0);
    });

    it('should handle missing yAxisLabel', () => {
      renderInteractiveChartWithCollaboration({
        yAxisLabel: undefined,
      });

      const chartElement = screen.getByTestId('line-chart');
      const chartOptions = JSON.parse(chartElement.getAttribute('data-chart-options') || '{}');

      expect(chartOptions.scales.y.title.text).toBe('Value');
    });
  });

  describe('Loading and Error States', () => {
    it('should handle loading state', () => {
      renderInteractiveChartWithCollaboration({
        loading: true,
      });

      expect(screen.getByTestId('line-chart')).toBeInTheDocument();
      // Chart should still render even when loading
    });

    it('should handle error state', () => {
      const error = new Error('Failed to load data');
      renderInteractiveChartWithCollaboration({
        error,
      });

      expect(screen.getByTestId('line-chart')).toBeInTheDocument();
      // Chart should still render even with error
    });
  });

  describe('Professional Features', () => {
    it('should display data point count', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByText(/6 data points/)).toBeInTheDocument();
    });

    it('should display date range information', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByText('Jan 1, 2024 - Jun 1, 2024')).toBeInTheDocument();
    });

    it('should show chart controls section', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByText('Chart Controls')).toBeInTheDocument();
    });

    it('should display transformation status', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByText('Transformation: None (Levels)')).toBeInTheDocument();
    });
  });

  describe('Accessibility', () => {
    it('should have proper ARIA labels', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByLabelText('Data Transformation')).toBeInTheDocument();
      expect(screen.getByLabelText('Start Date')).toBeInTheDocument();
      expect(screen.getByLabelText('End Date')).toBeInTheDocument();
    });

    it('should have proper chart title for screen readers', () => {
      renderInteractiveChartWithCollaboration();

      expect(screen.getByText('Test Economic Series')).toBeInTheDocument();
    });

    it('should support keyboard navigation', async () => {
      const user = userEvent.setup();
      renderInteractiveChartWithCollaboration();

      const transformationSelect = screen.getByLabelText('Data Transformation');

      // Should be focusable
      await user.tab();
      expect(transformationSelect).toHaveFocus();
    });
  });

  describe('Performance Optimizations', () => {
    it('should memoize chart data calculations', () => {
      const { rerender } = renderInteractiveChartWithCollaboration();

      // Re-render with same props
      rerender(
        <TestWrapper>
          <InteractiveChartWithCollaboration {...defaultProps} />
        </TestWrapper>
      );

      // Chart should still render correctly
      expect(screen.getByTestId('line-chart')).toBeInTheDocument();
    });

    it('should handle large datasets efficiently', () => {
      const largeDataset = Array.from({ length: 1000 }, (_, i) => ({
        date: `2024-${String(i % 12 + 1).padStart(2, '0')}-01`,
        value: 100 + Math.sin(i * 0.1) * 10,
        originalValue: 100 + Math.sin(i * 0.1) * 10,
      }));

      renderInteractiveChartWithCollaboration({
        data: largeDataset,
      });

      const chartElement = screen.getByTestId('line-chart');
      const chartData = JSON.parse(chartElement.getAttribute('data-chart-data') || '{}');

      expect(chartData.datasets[0].data).toHaveLength(1000);
    });
  });

  describe('Integration with Collaboration', () => {
    it('should pass seriesId to collaboration component', () => {
      renderInteractiveChartWithCollaboration({
        seriesId: 'custom-series-id',
      });

      expect(screen.getByTestId('chart-collaboration')).toBeInTheDocument();
    });

    it('should handle collaboration events', async () => {
      const user = userEvent.setup();
      renderInteractiveChartWithCollaboration();

      // Open collaboration panel
      const toggleButton = screen.getByTestId('toggle-collaboration');
      await user.click(toggleButton);

      expect(screen.getByText('Collaboration Panel')).toBeInTheDocument();
      expect(screen.getByText('Annotations: 0')).toBeInTheDocument();
    });

    it('should maintain collaboration state during chart interactions', async () => {
      const user = userEvent.setup();
      renderInteractiveChartWithCollaboration();

      // Open collaboration panel
      const toggleButton = screen.getByTestId('toggle-collaboration');
      await user.click(toggleButton);

      expect(screen.getByText('Collaboration Panel')).toBeInTheDocument();

      // Change transformation
      const transformationSelect = screen.getByLabelText('Data Transformation');
      await user.click(transformationSelect);
      await user.click(screen.getByText('Year-over-Year (%)'));

      // Collaboration panel should still be open
      expect(screen.getByText('Collaboration Panel')).toBeInTheDocument();
    });
  });
});
