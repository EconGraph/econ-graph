/**
 * Private Chart API for MCP Server Integration
 *
 * This API is only accessible from the backend/MCP server within the private network.
 * It provides chart generation capabilities using the existing frontend charting components.
 */

import { ChartConfiguration, ChartData } from 'chart.js';

export interface ChartSeriesData {
  id: string;
  name: string;
  dataPoints: Array<{
    date: string;
    value: number;
  }>;
  color?: string;
  type?: 'line' | 'bar' | 'scatter';
}

export interface ChartRequest {
  seriesData: ChartSeriesData[];
  chartType: 'line' | 'bar' | 'scatter';
  title?: string;
  width?: number;
  height?: number;
  startDate?: string;
  endDate?: string;
  showLegend?: boolean;
  showGrid?: boolean;
  yAxisLabel?: string;
  xAxisLabel?: string;
}

export interface ChartResponse {
  success: boolean;
  chartConfig?: ChartConfiguration;
  chartData?: ChartData;
  error?: string;
  metadata?: {
    seriesCount: number;
    dataPointCount: number;
    dateRange: {
      start: string;
      end: string;
    };
  };
}

/**
 * Generate chart configuration for Chart.js
 * This creates the same chart configuration used by the frontend components
 */
export function generateChartConfig(request: ChartRequest): ChartResponse {
  try {
    const {
      seriesData,
      chartType,
      title,
      showLegend = true,
      showGrid = true,
      yAxisLabel,
      xAxisLabel,
    } = request;

    // Validate input
    if (!seriesData || seriesData.length === 0) {
      return {
        success: false,
        error: 'No series data provided',
      };
    }

    // Process data points
    const processedSeries = seriesData.map(series => {
      const sortedPoints = series.dataPoints
        .sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime())
        .map(point => ({
          x: point.date,
          y: point.value,
        }));

      return {
        label: series.name,
        data: sortedPoints,
        borderColor: series.color || getDefaultColor(series.id),
        backgroundColor: series.color ? `${series.color}20` : `${getDefaultColor(series.id)}20`,
        fill: chartType === 'line' ? false : true,
        tension: 0.1,
        pointRadius: 2,
        pointHoverRadius: 4,
      };
    });

    // Calculate metadata
    const allDates = seriesData.flatMap(s => s.dataPoints.map(p => p.date));
    const uniqueDates = [...new Set(allDates)].sort();
    const totalDataPoints = seriesData.reduce((sum, s) => sum + s.dataPoints.length, 0);

    const chartData: ChartData = {
      datasets: processedSeries as any,
    };

    const chartConfig: ChartConfiguration = {
      type: chartType,
      data: chartData,
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          title: {
            display: !!title,
            text: title,
            font: {
              size: 16,
              weight: 'bold',
            },
          },
          legend: {
            display: showLegend,
            position: 'top' as const,
          },
          tooltip: {
            mode: 'index' as const,
            intersect: false,
            callbacks: {
              title: context => {
                const date = new Date(context[0].parsed.x);
                return date.toLocaleDateString();
              },
              label: context => {
                const value = context.parsed.y;
                const seriesName = context.dataset.label;
                return `${seriesName}: ${value.toLocaleString()}`;
              },
            },
          },
        },
        scales: {
          x: {
            display: true,
            title: {
              display: !!xAxisLabel,
              text: xAxisLabel || 'Date',
            },
            type: 'time' as const,
            time: {
              parser: 'YYYY-MM-DD',
              displayFormats: {
                day: 'MMM DD',
                month: 'MMM YYYY',
                year: 'YYYY',
              },
            },
            grid: {
              display: showGrid,
            },
          },
          y: {
            display: true,
            title: {
              display: !!yAxisLabel,
              text: yAxisLabel || 'Value',
            },
            grid: {
              display: showGrid,
            },
            ticks: {
              callback: function (value) {
                return typeof value === 'number' ? value.toLocaleString() : value;
              },
            },
          },
        },
        interaction: {
          mode: 'nearest' as const,
          axis: 'x' as const,
          intersect: false,
        },
      },
    };

    return {
      success: true,
      chartConfig,
      chartData,
      metadata: {
        seriesCount: seriesData.length,
        dataPointCount: totalDataPoints,
        dateRange: {
          start: uniqueDates[0] || '',
          end: uniqueDates[uniqueDates.length - 1] || '',
        },
      },
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error occurred',
    };
  }
}

/**
 * Get default color for series
 */
function getDefaultColor(seriesId: string): string {
  const colors = [
    '#1976d2', // Blue
    '#d32f2f', // Red
    '#388e3c', // Green
    '#f57c00', // Orange
    '#7b1fa2', // Purple
    '#00796b', // Teal
    '#c2185b', // Pink
    '#5d4037', // Brown
    '#455a64', // Blue Grey
    '#e64a19', // Deep Orange
  ];

  // Use series ID to consistently assign colors
  const hash = seriesId.split('').reduce((a, b) => {
    a = (a << 5) - a + b.charCodeAt(0);
    return a & a;
  }, 0);

  return colors[Math.abs(hash) % colors.length];
}

/**
 * Convert chart configuration to base64 image (for future use)
 * This would require a headless browser or canvas rendering
 */
export async function generateChartImage(chartConfig: ChartConfiguration): Promise<string> {
  // This would be implemented with a headless browser or server-side canvas
  // For now, we return the chart configuration that can be rendered client-side
  throw new Error(
    'Chart image generation not yet implemented - use chartConfig for client-side rendering'
  );
}

/**
 * Validate chart request
 */
export function validateChartRequest(request: any): boolean {
  if (!request) {
    return false;
  }

  return (
    Array.isArray(request.seriesData) &&
    request.seriesData.length > 0 &&
    typeof request.chartType === 'string' &&
    ['line', 'bar', 'scatter'].includes(request.chartType)
  );
}
