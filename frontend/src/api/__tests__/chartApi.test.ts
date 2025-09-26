/**
 * Tests for Private Chart API
 *
 * Tests the chart configuration generation functionality
 * used by the MCP server for professional chart creation.
 */

import { generateChartConfig, validateChartRequest, ChartRequest } from '../chartApi';
// import { generateChartImage } from '../chartApi';

describe('Chart API', () => {
  const mockSeriesData = [
    {
      id: 'series-1',
      name: 'GDP',
      dataPoints: [
        { date: '2020-01-01', value: 100.0 },
        { date: '2020-02-01', value: 101.5 },
        { date: '2020-03-01', value: 102.3 }
      ],
      color: '#1976d2',
      type: 'line' as const
    },
    {
      id: 'series-2',
      name: 'Unemployment Rate',
      dataPoints: [
        { date: '2020-01-01', value: 3.5 },
        { date: '2020-02-01', value: 3.6 },
        { date: '2020-03-01', value: 3.8 }
      ],
      color: '#d32f2f',
      type: 'line' as const
    }
  ];

  describe('generateChartConfig', () => {
    it('should generate valid line chart configuration', () => {
      const request: ChartRequest = {
        seriesData: mockSeriesData,
        chartType: 'line',
        title: 'GDP vs Unemployment Rate',
        showLegend: true,
        showGrid: true,
        yAxisLabel: 'Value',
        xAxisLabel: 'Date'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      expect(result.config).toBeDefined();
      expect(result.metadata).toBeDefined();

      // Check chart configuration structure
      expect(result.config?.type).toBe('line');
      expect(result.config?.data.datasets).toHaveLength(2);
      expect(result.config?.options?.plugins?.title?.text).toBe('GDP vs Unemployment Rate');
    });

    it('should generate valid bar chart configuration', () => {
      const request: ChartRequest = {
        seriesData: mockSeriesData,
        chartType: 'bar',
        title: 'Economic Indicators',
        showLegend: true,
        showGrid: true
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      expect(result.config?.type).toBe('bar');
      expect(result.config?.data.datasets).toHaveLength(2);
    });

    it('should generate valid scatter chart configuration', () => {
      const request: ChartRequest = {
        seriesData: mockSeriesData,
        chartType: 'scatter',
        title: 'Correlation Analysis',
        showLegend: true,
        showGrid: true
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      expect(result.config?.type).toBe('scatter');
      expect(result.config?.data.datasets).toHaveLength(2);
    });

    it('should handle empty series data', () => {
      const request: ChartRequest = {
        seriesData: [],
        chartType: 'line',
        title: 'Empty Chart'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No series data provided');
    });

    it('should handle missing series data', () => {
      const request = {
        chartType: 'line',
        title: 'Missing Data'
      } as any;

      const result = generateChartConfig(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No series data provided');
    });

    it('should generate proper metadata', () => {
      const request: ChartRequest = {
        seriesData: mockSeriesData,
        chartType: 'line',
        title: 'Test Chart'
      };

      const result = generateChartConfig(request);

      expect(result.metadata).toEqual({
        seriesCount: 2,
        dataPointCount: 6, // 3 points per series
        chartType: 'line'
      });
    });

    it('should handle unsorted data points', () => {
      const unsortedData = [
        {
          id: 'series-1',
          name: 'GDP',
          dataPoints: [
            { date: '2020-03-01', value: 102.3 },
            { date: '2020-01-01', value: 100.0 },
            { date: '2020-02-01', value: 101.5 }
          ]
        }
      ];

      const request: ChartRequest = {
        seriesData: unsortedData,
        chartType: 'line',
        title: 'Unsorted Data'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      expect(result.metadata?.seriesCount).toBe(1);
      expect(result.metadata?.dataPointCount).toBe(3);
    });

    it('should apply custom colors correctly', () => {
      const request: ChartRequest = {
        seriesData: mockSeriesData,
        chartType: 'line',
        title: 'Custom Colors'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      const datasets = result.config?.data.datasets;
      expect(datasets?.[0].borderColor).toBe('#1976d2');
      expect(datasets?.[1].borderColor).toBe('#d32f2f');
    });

    it('should handle missing colors with defaults', () => {
      const dataWithoutColors = [
        {
          id: 'series-1',
          name: 'GDP',
          dataPoints: [
            { date: '2020-01-01', value: 100.0 }
          ]
        }
      ];

      const request: ChartRequest = {
        seriesData: dataWithoutColors,
        chartType: 'line',
        title: 'Default Colors'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      const datasets = result.config?.data.datasets;
      expect(datasets?.[0].borderColor).toBeDefined();
      expect(datasets?.[0].backgroundColor).toBeDefined();
    });

    it('should configure chart options correctly', () => {
      const request: ChartRequest = {
        seriesData: mockSeriesData,
        chartType: 'line',
        title: 'Configured Chart',
        showLegend: false,
        showGrid: false,
        yAxisLabel: 'Billions USD',
        xAxisLabel: 'Time Period'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      const options = result.config?.options;
      expect(options?.plugins?.legend?.display).toBe(false);
      expect(options?.scales?.x?.grid?.display).toBe(false);
      expect(options?.scales?.y?.grid?.display).toBe(false);
      expect((options?.scales?.y as any)?.title?.text).toBe('Billions USD');
      expect((options?.scales?.x as any)?.title?.text).toBe('Time Period');
    });
  });

  describe('validateChartRequest', () => {
    it('should validate correct chart request', () => {
      const validRequest = {
        seriesData: mockSeriesData,
        chartType: 'line'
      };

      expect(validateChartRequest(validRequest)).toBe(true);
    });

    it('should validate bar chart request', () => {
      const validRequest = {
        seriesData: mockSeriesData,
        chartType: 'bar'
      };

      expect(validateChartRequest(validRequest)).toBe(true);
    });

    it('should validate scatter chart request', () => {
      const validRequest = {
        seriesData: mockSeriesData,
        chartType: 'scatter'
      };

      expect(validateChartRequest(validRequest)).toBe(true);
    });

    it('should reject request without seriesData', () => {
      const invalidRequest = {
        chartType: 'line'
      };

      expect(validateChartRequest(invalidRequest)).toBe(false);
    });

    it('should reject request with empty seriesData', () => {
      const invalidRequest = {
        seriesData: [],
        chartType: 'line'
      };

      expect(validateChartRequest(invalidRequest)).toBe(false);
    });

    it('should reject request without chartType', () => {
      const invalidRequest = {
        seriesData: mockSeriesData
      };

      expect(validateChartRequest(invalidRequest)).toBe(false);
    });

    it('should reject request with invalid chartType', () => {
      const invalidRequest = {
        seriesData: mockSeriesData,
        chartType: 'pie'
      };

      expect(validateChartRequest(invalidRequest)).toBe(false);
    });

    it('should reject null request', () => {
      expect(validateChartRequest(null)).toBe(false);
    });

    it('should reject undefined request', () => {
      expect(validateChartRequest(undefined)).toBe(false);
    });
  });

  describe('Error Handling', () => {
    it('should handle malformed data points', () => {
      const malformedData = [
        {
          id: 'series-1',
          name: 'GDP',
          dataPoints: [
            { date: 'invalid-date', value: 'not-a-number' },
            { date: '2020-01-01', value: 100.0 }
          ]
        }
      ];

      const request: ChartRequest = {
        seriesData: malformedData as any,
        chartType: 'line',
        title: 'Malformed Data'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true); // Should still succeed but handle invalid data gracefully
      expect(result.config?.data.datasets[0].data).toHaveLength(2);
    });

    it('should handle missing data point properties', () => {
      const incompleteData = [
        {
          id: 'series-1',
          name: 'GDP',
          dataPoints: [
            { date: '2020-01-01' }, // Missing value
            { value: 100.0 }, // Missing date
            { date: '2020-02-01', value: 101.5 }
          ]
        }
      ];

      const request: ChartRequest = {
        seriesData: incompleteData as any,
        chartType: 'line',
        title: 'Incomplete Data'
      };

      const result = generateChartConfig(request);

      expect(result.success).toBe(true);
      expect(result.config?.data.datasets[0].data).toHaveLength(3);
    });
  });
});
