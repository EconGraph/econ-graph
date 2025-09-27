/**
 * Chart API utilities for generating and managing chart data
 */

export interface ChartDataPoint {
  x: number;
  y: number;
  label?: string;
}

export interface ChartSeries {
  label: string;
  data: ChartDataPoint[];
  color?: string;
  type?: 'line' | 'bar' | 'scatter';
}

export interface ChartConfig {
  title: string;
  xAxisLabel: string;
  yAxisLabel: string;
  width: number;
  height: number;
  showLegend: boolean;
}

export interface ChartRequest {
  seriesData: any[];
  chartType: 'line' | 'bar' | 'scatter';
  title?: string;
  colors?: string[];
  showLegend?: boolean;
  showGrid?: boolean;
  yAxisLabel?: string;
  xAxisLabel?: string;
  options?: any;
}

export interface ChartResponse {
  success: boolean;
  config?: any;
  metadata?: any;
  error?: string;
}

/**
 * Generate chart data from series data
 */
export function generateChartData(
  seriesData: any[],
  config: Partial<ChartConfig> = {}
): { series: ChartSeries[]; config: ChartConfig } {
  const defaultConfig: ChartConfig = {
    title: 'Chart',
    xAxisLabel: 'Date',
    yAxisLabel: 'Value',
    width: 800,
    height: 400,
    showLegend: true,
    ...config,
  };

  const series: ChartSeries[] = seriesData.map((data, index) => ({
    label: data.title || `Series ${index + 1}`,
    data:
      data.dataPoints?.map((point: any, i: number) => ({
        x: i,
        y: point.value,
        label: point.date,
      })) || [],
    color: `hsl(${(index * 137.5) % 360}, 70%, 50%)`,
    type: 'line',
  }));

  return { series, config: defaultConfig };
}

/**
 * Export chart data to various formats
 */
export function exportChartData(
  series: ChartSeries[],
  format: 'json' | 'csv' | 'excel' = 'json'
): string | Blob {
  switch (format) {
    case 'json':
      return JSON.stringify(series, null, 2);
    case 'csv':
      // Simple CSV export
      const headers = ['Series', 'X', 'Y', 'Label'];
      const rows = series.flatMap(s =>
        s.data.map(point => [s.label, point.x, point.y, point.label || ''])
      );
      return [headers, ...rows].map(row => row.join(',')).join('\n');
    case 'excel':
      // For Excel, we'd need a library like xlsx
      // For now, return CSV format
      return exportChartData(series, 'csv') as string;
    default:
      return JSON.stringify(series, null, 2);
  }
}

/**
 * Validate chart configuration
 */
export function validateChartConfig(config: Partial<ChartConfig>): string[] {
  const errors: string[] = [];

  if (config.width && config.width < 100) {
    errors.push('Width must be at least 100px');
  }

  if (config.height && config.height < 100) {
    errors.push('Height must be at least 100px');
  }

  if (config.title && config.title.length > 100) {
    errors.push('Title must be 100 characters or less');
  }

  return errors;
}

/**
 * Generate chart configuration from request
 */
export function generateChartConfig(request: ChartRequest): ChartResponse {
  try {
    if (!request.seriesData || request.seriesData.length === 0) {
      return {
        success: false,
        error: 'No series data provided',
      };
    }

    if (!request.chartType) {
      return {
        success: false,
        error: 'Chart type is required',
      };
    }

    const config = {
      type: request.chartType,
      data: {
        labels: request.seriesData[0]?.dataPoints?.map((point: any) => point.date) || [],
        datasets: request.seriesData.map((series, index) => ({
          label: series.title || `Series ${index + 1}`,
          data: series.dataPoints?.map((point: any) => point.value) || [],
          backgroundColor:
            request.colors?.[index] || (series as any).color || `hsl(${index * 137.5}, 70%, 50%)`,
          borderColor:
            request.colors?.[index] || (series as any).color || `hsl(${index * 137.5}, 70%, 50%)`,
          borderWidth: 2,
          fill: request.chartType === 'line',
        })),
      },
      options: {
        responsive: true,
        plugins: {
          title: {
            display: true,
            text: request.title || 'Chart',
          },
          legend: {
            display: (request as any).showLegend !== false,
          },
        },
        scales: {
          x: {
            display: true,
            title: {
              display: true,
              text: (request as any).xAxisLabel || 'Date',
            },
            grid: {
              display: (request as any).showGrid !== false,
            },
          },
          y: {
            display: true,
            title: {
              display: true,
              text: (request as any).yAxisLabel || 'Value',
            },
            grid: {
              display: (request as any).showGrid !== false,
            },
          },
        },
        ...request.options,
      },
    };

    return {
      success: true,
      config,
      metadata: {
        seriesCount: request.seriesData.length,
        dataPointCount: request.seriesData.reduce(
          (sum, series) => sum + (series.dataPoints?.length || 0),
          0
        ),
        chartType: request.chartType,
      },
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

/**
 * Validate chart request
 */
export function validateChartRequest(request: any): boolean {
  if (!request) return false;
  if (!request.seriesData || !Array.isArray(request.seriesData) || request.seriesData.length === 0)
    return false;
  if (!request.chartType || !['line', 'bar', 'scatter'].includes(request.chartType)) return false;
  return true;
}
