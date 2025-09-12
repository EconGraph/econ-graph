/**
 * Chart API Core Functions
 *
 * Provides chart configuration generation using Chart.js
 * for the EconGraph MCP server integration.
 */

const { Chart } = require('chart.js');

/**
 * Generate chart configuration for Chart.js
 * This creates the same chart configuration used by the frontend components
 */
function generateChartConfig(request) {
  try {
    const { seriesData, chartType, title, showLegend = true, showGrid = true, yAxisLabel, xAxisLabel } = request;

    // Validate input
    if (!seriesData || seriesData.length === 0) {
      return {
        success: false,
        error: 'No series data provided'
      };
    }

    // Process data points
    const processedSeries = seriesData.map(series => {
      const sortedPoints = series.dataPoints
        .sort((a, b) => new Date(a.date).getTime() - new Date(b.date).getTime())
        .map(point => ({
          x: point.date,
          y: point.value
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

    const chartData = {
      datasets: processedSeries
    };

    const chartConfig = {
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
              weight: 'bold'
            }
          },
          legend: {
            display: showLegend,
            position: 'top',
          },
          tooltip: {
            mode: 'index',
            intersect: false,
            callbacks: {
              title: (context) => {
                const date = new Date(context[0].parsed.x);
                return date.toLocaleDateString();
              },
              label: (context) => {
                const value = context.parsed.y;
                const seriesName = context.dataset.label;
                return `${seriesName}: ${value.toLocaleString()}`;
              }
            }
          }
        },
        scales: {
          x: {
            display: true,
            title: {
              display: !!xAxisLabel,
              text: xAxisLabel || 'Date'
            },
            type: 'time',
            time: {
              parser: 'YYYY-MM-DD',
              displayFormats: {
                day: 'MMM DD',
                month: 'MMM YYYY',
                year: 'YYYY'
              }
            },
            grid: {
              display: showGrid
            }
          },
          y: {
            display: true,
            title: {
              display: !!yAxisLabel,
              text: yAxisLabel || 'Value'
            },
            grid: {
              display: showGrid
            },
            ticks: {
              callback: function(value) {
                return typeof value === 'number' ? value.toLocaleString() : value;
              }
            }
          }
        },
        interaction: {
          mode: 'nearest',
          axis: 'x',
          intersect: false
        }
      }
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
          end: uniqueDates[uniqueDates.length - 1] || ''
        }
      }
    };

  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : 'Unknown error occurred'
    };
  }
}

/**
 * Get default color for series
 */
function getDefaultColor(seriesId) {
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
    a = ((a << 5) - a) + b.charCodeAt(0);
    return a & a;
  }, 0);

  return colors[Math.abs(hash) % colors.length];
}

/**
 * Validate chart request
 */
function validateChartRequest(request) {
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

module.exports = {
  generateChartConfig,
  validateChartRequest,
  getDefaultColor
};
