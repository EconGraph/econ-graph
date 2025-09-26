/**
 * Chart API utilities for generating and managing chart data (CommonJS version)
 */

/**
 * Generate chart configuration from request
 */
function generateChartConfig(request) {
  try {
    if (!request.seriesData || request.seriesData.length === 0) {
      return {
        success: false,
        error: 'No series data provided'
      };
    }

    if (!request.chartType) {
      return {
        success: false,
        error: 'Chart type is required'
      };
    }

    const config = {
      type: request.chartType,
      data: {
        labels: request.seriesData[0]?.dataPoints?.map((point) => point.date) || [],
        datasets: request.seriesData.map((series, index) => ({
          label: series.title || `Series ${index + 1}`,
          data: series.dataPoints?.map((point) => point.value) || [],
          backgroundColor: request.colors?.[index] || `hsl(${index * 137.5}, 70%, 50%)`,
          borderColor: request.colors?.[index] || `hsl(${index * 137.5}, 70%, 50%)`,
          borderWidth: 2,
          fill: request.chartType === 'line'
        }))
      },
      options: {
        responsive: true,
        plugins: {
          title: {
            display: true,
            text: request.title || 'Chart'
          },
          legend: {
            display: true
          }
        },
        scales: {
          x: {
            display: true,
            title: {
              display: true,
              text: 'Date'
            }
          },
          y: {
            display: true,
            title: {
              display: true,
              text: 'Value'
            }
          }
        },
        ...request.options
      }
    };

    return {
      success: true,
      config,
      metadata: {
        seriesCount: request.seriesData.length,
        dataPointCount: request.seriesData.reduce((sum, series) => sum + (series.dataPoints?.length || 0), 0),
        chartType: request.chartType
      }
    };
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error'
    };
  }
}

/**
 * Validate chart request
 */
function validateChartRequest(request) {
  if (!request) return false;
  if (!request.seriesData || !Array.isArray(request.seriesData) || request.seriesData.length === 0) return false;
  if (!request.chartType || !['line', 'bar', 'scatter'].includes(request.chartType)) return false;
  return true;
}

module.exports = {
  generateChartConfig,
  validateChartRequest
};
