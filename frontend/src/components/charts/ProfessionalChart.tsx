/**
 * REQUIREMENT: Professional chart analytics with Bloomberg Terminal-level capabilities
 * PURPOSE: Advanced charting component with technical analysis, multi-series overlay, and annotations
 * This provides sophisticated economic analysis tools for professional users.
 */

import React, { useState, useMemo, useCallback } from 'react';
import {
  Box,
  Paper,
  Typography,
  Checkbox,
  FormControlLabel,
  Chip,
  Grid,
  IconButton,
  Tooltip,
  Accordion,
  AccordionSummary,
  AccordionDetails,
} from '@mui/material';
import {
  TrendingUp as TrendingUpIcon,
  ExpandMore as ExpandMoreIcon,
  Add as AddIcon,
  Fullscreen as FullscreenIcon,
  GetApp as ExportIcon,
} from '@mui/icons-material';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip as ChartTooltip,
  Legend,
  Filler,
  ChartOptions,
  ChartData,
} from 'chart.js';
import { Line } from 'react-chartjs-2';
import annotationPlugin from 'chartjs-plugin-annotation';
import {
  calculateSMA,
  calculateEMA,
  calculateBollingerBands,
  calculateRSI,
  calculateROC,
  calculateStandardDeviation,
  detectEconomicCycles,
  calculateCorrelation,
  getEconomicEventsInRange,
  DataPoint,
  TechnicalIndicator,
  BollingerBands,
  RSIPoint,
  CyclePoint,
} from '../../utils/technicalAnalysis';

// Register Chart.js components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  ChartTooltip,
  Legend,
  Filler,
  annotationPlugin
);

export interface SeriesData {
  id: string;
  title: string;
  description: string;
  data: DataPoint[];
  color: string;
  unit: string;
  frequency: string;
}

export interface ChartAnnotation {
  id: string;
  date: string;
  value?: number;
  title: string;
  description: string;
  color: string;
  type: 'line' | 'point' | 'box';
}

interface ProfessionalChartProps {
  primarySeries: SeriesData;
  secondarySeries?: SeriesData[];
  height?: number;
  showTechnicalAnalysis?: boolean;
  showEconomicEvents?: boolean;
  allowAnnotations?: boolean;
  onAnnotationAdd?: (annotation: ChartAnnotation) => void;
  onSeriesAdd?: () => void;
}

interface TechnicalAnalysisSettings {
  sma: { enabled: boolean; periods: number[] };
  ema: { enabled: boolean; periods: number[] };
  bollinger: { enabled: boolean; period: number; stdDev: number };
  rsi: { enabled: boolean; period: number };
  roc: { enabled: boolean; period: number };
  stdDev: { enabled: boolean; period: number };
  cycles: { enabled: boolean; lookback: number };
}

const ProfessionalChart: React.FC<ProfessionalChartProps> = ({
  primarySeries,
  secondarySeries = [],
  height = 600,
  showTechnicalAnalysis = true,
  showEconomicEvents = true,
  allowAnnotations: _allowAnnotations = true,
  onAnnotationAdd: _onAnnotationAdd,
  onSeriesAdd,
}) => {
  const [taSettings, setTASettings] = useState<TechnicalAnalysisSettings>({
    sma: { enabled: false, periods: [20, 50] },
    ema: { enabled: false, periods: [12, 26] },
    bollinger: { enabled: false, period: 20, stdDev: 2 },
    rsi: { enabled: false, period: 14 },
    roc: { enabled: false, period: 12 },
    stdDev: { enabled: false, period: 20 },
    cycles: { enabled: false, lookback: 6 },
  });

  const [showEvents, setShowEvents] = useState(showEconomicEvents);
  const [showCorrelation, setShowCorrelation] = useState(false);
  const [customAnnotations] = useState<ChartAnnotation[]>([]); // Custom chart annotations
  const [isFullscreen, setIsFullscreen] = useState(false);

  // Calculate technical indicators
  const technicalIndicators = useMemo(() => {
    const indicators: {
      [key: string]: TechnicalIndicator[] | BollingerBands[] | RSIPoint[] | CyclePoint[];
    } = {};

    if (taSettings.sma.enabled) {
      taSettings.sma.periods.forEach(period => {
        indicators[`SMA${period}`] = calculateSMA(primarySeries.data, period);
      });
    }

    if (taSettings.ema.enabled) {
      taSettings.ema.periods.forEach(period => {
        indicators[`EMA${period}`] = calculateEMA(primarySeries.data, period);
      });
    }

    if (taSettings.bollinger.enabled) {
      indicators.bollinger = calculateBollingerBands(
        primarySeries.data,
        taSettings.bollinger.period,
        taSettings.bollinger.stdDev
      );
    }

    if (taSettings.rsi.enabled) {
      indicators.rsi = calculateRSI(primarySeries.data, taSettings.rsi.period);
    }

    if (taSettings.roc.enabled) {
      indicators.roc = calculateROC(primarySeries.data, taSettings.roc.period);
    }

    if (taSettings.stdDev.enabled) {
      indicators.stdDev = calculateStandardDeviation(primarySeries.data, taSettings.stdDev.period);
    }

    if (taSettings.cycles.enabled) {
      indicators.cycles = detectEconomicCycles(primarySeries.data, taSettings.cycles.lookback);
    }

    return indicators;
  }, [primarySeries.data, taSettings]);

  // Get economic events for the date range
  const economicEvents = useMemo(() => {
    if (
      !showEvents ||
      !primarySeries.data ||
      !Array.isArray(primarySeries.data) ||
      primarySeries.data.length === 0
    )
      return [];

    const startDate = primarySeries.data[0].date;
    const endDate = primarySeries.data[primarySeries.data.length - 1].date;

    return getEconomicEventsInRange(startDate, endDate);
  }, [primarySeries.data, showEvents]);

  // Calculate correlations with secondary series
  const correlations = useMemo(() => {
    if (!showCorrelation || secondarySeries.length === 0) return [];

    return secondarySeries.map(series => ({
      seriesId: series.id,
      title: series.title,
      correlation: calculateCorrelation(primarySeries.data, series.data),
    }));
  }, [primarySeries.data, secondarySeries, showCorrelation]);

  // Prepare chart data
  const chartData: ChartData<'line'> = useMemo(() => {
    // Ensure primary series data exists and is valid
    if (
      !primarySeries.data ||
      !Array.isArray(primarySeries.data) ||
      primarySeries.data.length === 0
    ) {
      return { labels: [], datasets: [] };
    }

    const labels = primarySeries.data.map(point => point.date);
    const datasets: any[] = [];

    // Primary series
    datasets.push({
      label: primarySeries.title,
      data: primarySeries.data.map(point => point.value),
      borderColor: primarySeries.color,
      backgroundColor: `${primarySeries.color}20`,
      borderWidth: 2,
      fill: false,
      tension: 0.1,
      pointRadius: 1,
      pointHoverRadius: 4,
      yAxisID: 'y',
    });

    // Secondary series - ensure it exists and is an array
    if (secondarySeries && Array.isArray(secondarySeries)) {
      secondarySeries.forEach((series, index) => {
        if (series.data && Array.isArray(series.data)) {
          datasets.push({
            label: series.title,
            data: series.data.map(point => point.value),
            borderColor: series.color,
            backgroundColor: `${series.color}20`,
            borderWidth: 2,
            fill: false,
            tension: 0.1,
            pointRadius: 1,
            pointHoverRadius: 4,
            yAxisID: index === 0 && secondarySeries.length > 0 ? 'y1' : 'y',
            borderDash: index % 2 === 1 ? [5, 5] : undefined,
          });
        }
      });
    }

    // Technical indicators
    Object.entries(technicalIndicators).forEach(([key, data]) => {
      if (key.startsWith('SMA') || key.startsWith('EMA')) {
        const indicators = data as TechnicalIndicator[];
        datasets.push({
          label: key,
          data: labels.map(date => {
            const indicator = indicators.find(i => i.date === date);
            return indicator ? indicator.value : null;
          }),
          borderColor: key.startsWith('SMA') ? '#ff6b6b' : '#4ecdc4',
          backgroundColor: 'transparent',
          borderWidth: 1,
          fill: false,
          pointRadius: 0,
          borderDash: [3, 3],
          yAxisID: 'y',
        });
      } else if (key === 'bollinger') {
        const bands = data as BollingerBands[];

        // Upper band
        datasets.push({
          label: 'Bollinger Upper',
          data: labels.map(date => {
            const band = bands.find(b => b.date === date);
            return band ? band.upper : null;
          }),
          borderColor: '#ffa726',
          backgroundColor: 'transparent',
          borderWidth: 1,
          fill: '+1',
          pointRadius: 0,
          yAxisID: 'y',
        });

        // Middle band (SMA)
        datasets.push({
          label: 'Bollinger Middle',
          data: labels.map(date => {
            const band = bands.find(b => b.date === date);
            return band ? band.middle : null;
          }),
          borderColor: '#ff9800',
          backgroundColor: 'transparent',
          borderWidth: 1,
          fill: false,
          pointRadius: 0,
          borderDash: [2, 2],
          yAxisID: 'y',
        });

        // Lower band
        datasets.push({
          label: 'Bollinger Lower',
          data: labels.map(date => {
            const band = bands.find(b => b.date === date);
            return band ? band.lower : null;
          }),
          borderColor: '#ffa726',
          backgroundColor: `#ffa72620`,
          borderWidth: 1,
          fill: '-1',
          pointRadius: 0,
          yAxisID: 'y',
        });
      }
    });

    return { labels, datasets };
  }, [primarySeries, secondarySeries, technicalIndicators]);

  // Chart options
  const chartOptions: ChartOptions<'line'> = useMemo(() => {
    const annotations: any = {};

    // Economic events annotations
    economicEvents.forEach((event, index) => {
      annotations[`event${index}`] = {
        type: 'line',
        xMin: event.date,
        xMax: event.date,
        borderColor:
          event.impact === 'high' ? '#f44336' : event.impact === 'medium' ? '#ff9800' : '#4caf50',
        borderWidth: 2,
        borderDash: [5, 5],
        label: {
          display: true,
          content: event.title,
          position: 'top',
          backgroundColor:
            event.impact === 'high' ? '#f44336' : event.impact === 'medium' ? '#ff9800' : '#4caf50',
          color: 'white',
          font: {
            size: 10,
          },
        },
      };
    });

    // Cycle annotations
    if (taSettings.cycles.enabled && technicalIndicators.cycles) {
      const cycles = technicalIndicators.cycles as CyclePoint[];
      cycles.forEach((cycle, index) => {
        annotations[`cycle${index}`] = {
          type: 'point',
          xValue: cycle.date,
          yValue: cycle.value,
          backgroundColor: cycle.type === 'peak' ? '#f44336' : '#4caf50',
          borderColor: cycle.type === 'peak' ? '#d32f2f' : '#388e3c',
          borderWidth: 2,
          radius: 6,
          label: {
            display: true,
            content: cycle.type === 'peak' ? '📈' : '📉',
            position: cycle.type === 'peak' ? 'top' : 'bottom',
          },
        };
      });
    }

    // Custom annotations - ensure it's an array before iterating
    if (Array.isArray(customAnnotations) && customAnnotations.length > 0) {
      customAnnotations.forEach((annotation: any, index: number) => {
        annotations[`custom${index}`] = {
          type: annotation.type,
          xMin: annotation.date,
          xMax: annotation.date,
          yMin: annotation.value,
          yMax: annotation.value,
          borderColor: annotation.color,
          backgroundColor: `${annotation.color}40`,
          borderWidth: 2,
          label: {
            display: true,
            content: annotation.title,
            position: 'top',
            backgroundColor: annotation.color,
            color: 'white',
          },
        };
      });
    }

    return {
      responsive: true,
      maintainAspectRatio: false,
      interaction: {
        mode: 'index' as const,
        intersect: false,
      },
      plugins: {
        title: {
          display: true,
          text: `Professional Analysis: ${primarySeries.title}`,
          font: {
            size: 16,
            weight: 'bold',
          },
        },
        tooltip: {
          callbacks: {
            title: context => {
              return new Date(context[0].label).toLocaleDateString();
            },
            label: context => {
              const value =
                typeof context.parsed.y === 'number' ? context.parsed.y.toFixed(2) : 'N/A';
              return `${context.dataset.label}: ${value}`;
            },
            afterBody: context => {
              const date = context[0].label;
              const event = economicEvents.find(e => e.date === date);
              return event ? [`📅 ${event.title}`, event.description] : [];
            },
          },
        },
        legend: {
          display: true,
          position: 'top' as const,
          labels: {
            usePointStyle: true,
            padding: 20,
          },
        },
        annotation: {
          annotations,
        },
      },
      scales: {
        x: {
          display: true,
          title: {
            display: true,
            text: 'Date',
          },
          type: 'category',
        },
        y: {
          type: 'linear' as const,
          display: true,
          position: 'left' as const,
          title: {
            display: true,
            text: primarySeries.unit,
          },
        },
        y1:
          secondarySeries.length > 0
            ? {
                type: 'linear' as const,
                display: true,
                position: 'right' as const,
                title: {
                  display: true,
                  text: secondarySeries[0]?.unit || '',
                },
                grid: {
                  drawOnChartArea: false,
                },
              }
            : undefined,
      },
    };
  }, [
    primarySeries,
    secondarySeries,
    economicEvents,
    technicalIndicators,
    taSettings,
    customAnnotations,
  ]);

  const handleTASettingChange = useCallback(
    (indicator: keyof TechnicalAnalysisSettings, setting: string, value: any) => {
      setTASettings(prev => ({
        ...prev,
        [indicator]: {
          ...prev[indicator],
          [setting]: value,
        },
      }));
    },
    []
  );

  const exportChart = useCallback(() => {
    // Implementation for chart export (PNG, PDF, SVG)
    // Export functionality would be implemented here
  }, []);

  return (
    <Box sx={{ width: '100%', height: isFullscreen ? '100vh' : height }}>
      <Paper elevation={3} sx={{ p: 2, height: '100%', display: 'flex', flexDirection: 'column' }}>
        {/* Chart Controls */}
        <Box sx={{ mb: 2, display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: 1 }}>
          <Typography variant='h6' sx={{ flexGrow: 1 }}>
            Professional Chart Analytics
          </Typography>

          <Box sx={{ display: 'flex', gap: 1 }}>
            <Tooltip title='Add Series'>
              <IconButton onClick={onSeriesAdd} size='small'>
                <AddIcon />
              </IconButton>
            </Tooltip>

            <Tooltip title='Export Chart'>
              <IconButton onClick={exportChart} size='small'>
                <ExportIcon />
              </IconButton>
            </Tooltip>

            <Tooltip title='Fullscreen'>
              <IconButton onClick={() => setIsFullscreen(!isFullscreen)} size='small'>
                <FullscreenIcon />
              </IconButton>
            </Tooltip>
          </Box>
        </Box>

        {/* Technical Analysis Controls */}
        {showTechnicalAnalysis && (
          <Accordion sx={{ mb: 2 }}>
            <AccordionSummary expandIcon={<ExpandMoreIcon />}>
              <Typography variant='subtitle1'>
                <TrendingUpIcon sx={{ mr: 1, verticalAlign: 'middle' }} />
                Technical Analysis
              </Typography>
            </AccordionSummary>
            <AccordionDetails>
              <Grid container spacing={2}>
                {/* Moving Averages */}
                <Grid item xs={12} md={6}>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={taSettings.sma.enabled}
                        onChange={e => handleTASettingChange('sma', 'enabled', e.target.checked)}
                      />
                    }
                    label='Simple Moving Average (SMA)'
                  />
                  {taSettings.sma.enabled && (
                    <Box sx={{ ml: 3 }}>
                      <Typography variant='caption'>Periods: 20, 50</Typography>
                    </Box>
                  )}
                </Grid>

                <Grid item xs={12} md={6}>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={taSettings.ema.enabled}
                        onChange={e => handleTASettingChange('ema', 'enabled', e.target.checked)}
                      />
                    }
                    label='Exponential Moving Average (EMA)'
                  />
                  {taSettings.ema.enabled && (
                    <Box sx={{ ml: 3 }}>
                      <Typography variant='caption'>Periods: 12, 26</Typography>
                    </Box>
                  )}
                </Grid>

                {/* Bollinger Bands */}
                <Grid item xs={12} md={6}>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={taSettings.bollinger.enabled}
                        onChange={e =>
                          handleTASettingChange('bollinger', 'enabled', e.target.checked)
                        }
                      />
                    }
                    label='Bollinger Bands'
                  />
                  {taSettings.bollinger.enabled && (
                    <Box sx={{ ml: 3 }}>
                      <Typography variant='caption'>
                        Period: {taSettings.bollinger.period}, Std Dev:{' '}
                        {taSettings.bollinger.stdDev}
                      </Typography>
                    </Box>
                  )}
                </Grid>

                {/* Economic Cycles */}
                <Grid item xs={12} md={6}>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={taSettings.cycles.enabled}
                        onChange={e => handleTASettingChange('cycles', 'enabled', e.target.checked)}
                      />
                    }
                    label='Economic Cycle Detection'
                  />
                </Grid>

                {/* Economic Events */}
                <Grid item xs={12} md={6}>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={showEvents}
                        onChange={e => setShowEvents(e.target.checked)}
                      />
                    }
                    label='Economic Events'
                  />
                </Grid>

                {/* Correlation Analysis */}
                <Grid item xs={12} md={6}>
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={showCorrelation}
                        onChange={e => setShowCorrelation(e.target.checked)}
                      />
                    }
                    label='Correlation Analysis'
                  />
                </Grid>
              </Grid>
            </AccordionDetails>
          </Accordion>
        )}

        {/* Correlation Display */}
        {showCorrelation && correlations.length > 0 && (
          <Box sx={{ mb: 2 }}>
            <Typography variant='subtitle2' sx={{ mb: 1 }}>
              Correlation Analysis:
            </Typography>
            <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
              {correlations.map(corr => (
                <Chip
                  key={corr.seriesId}
                  label={`${corr.title}: ${corr.correlation.toFixed(3)}`}
                  color={Math.abs(corr.correlation) > 0.7 ? 'primary' : 'default'}
                  size='small'
                />
              ))}
            </Box>
          </Box>
        )}

        {/* Chart */}
        <Box sx={{ flexGrow: 1, position: 'relative' }}>
          <Line data={chartData} options={chartOptions} />
        </Box>

        {/* Economic Events Summary */}
        {showEvents && economicEvents.length > 0 && (
          <Box sx={{ mt: 2 }}>
            <Typography variant='subtitle2' sx={{ mb: 1 }}>
              Economic Events in Range:
            </Typography>
            <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
              {economicEvents.slice(0, 5).map((event, index) => (
                <Chip
                  key={index}
                  label={event.title}
                  size='small'
                  color={
                    event.impact === 'high'
                      ? 'error'
                      : event.impact === 'medium'
                        ? 'warning'
                        : 'success'
                  }
                />
              ))}
              {economicEvents.length > 5 && (
                <Chip
                  label={`+${economicEvents.length - 5} more`}
                  size='small'
                  variant='outlined'
                />
              )}
            </Box>
          </Box>
        )}
      </Paper>
    </Box>
  );
};

export default ProfessionalChart;
