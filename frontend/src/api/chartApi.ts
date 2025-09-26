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
    data: data.dataPoints?.map((point: any, i: number) => ({
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