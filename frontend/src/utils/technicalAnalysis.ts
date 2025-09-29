/**
 * REQUIREMENT: Professional chart analytics with technical analysis tools
 * PURPOSE: Provide Bloomberg Terminal-level technical analysis capabilities
 * This module implements statistical indicators commonly used in economic analysis.
 */

export interface DataPoint {
  date: string;
  value: number;
  originalValue?: number;
}

export interface TechnicalIndicator {
  date: string;
  value: number;
  indicator: string;
}

export interface BollingerBands {
  date: string;
  upper: number;
  middle: number;
  lower: number;
}

export interface RSIPoint {
  date: string;
  rsi: number;
}

/**
 * Calculate Simple Moving Average
 * Used for trend analysis and smoothing economic data.
 * @param data - Array of data points to analyze.
 * @param period - Number of periods for the moving average.
 * @returns Array of technical indicator values.
 */
export function calculateSMA(data: DataPoint[], period: number): TechnicalIndicator[] {
  if (data.length < period) return [];

  const sma: TechnicalIndicator[] = [];

  for (let i = period - 1; i < data.length; i++) {
    const slice = data.slice(i - period + 1, i + 1);
    const average = slice.reduce((sum, point) => sum + point.value, 0) / period;

    sma.push({
      date: data[i].date,
      value: average,
      indicator: `SMA(${period})`,
    });
  }

  return sma;
}

/**
 * Calculate Exponential Moving Average
 * More responsive to recent changes than SMA.
 * @param data - Array of data points to analyze.
 * @param period - Number of periods for the moving average.
 * @returns Array of technical indicator values.
 */
export function calculateEMA(data: DataPoint[], period: number): TechnicalIndicator[] {
  if (data.length === 0) return [];

  const ema: TechnicalIndicator[] = [];
  const multiplier = 2 / (period + 1);

  // Start with SMA for first value
  let previousEMA = data.slice(0, period).reduce((sum, point) => sum + point.value, 0) / period;

  for (let i = period - 1; i < data.length; i++) {
    const currentEMA = data[i].value * multiplier + previousEMA * (1 - multiplier);

    ema.push({
      date: data[i].date,
      value: currentEMA,
      indicator: `EMA(${period})`,
    });

    previousEMA = currentEMA;
  }

  return ema;
}

/**
 * Calculate Bollinger Bands
 * Statistical measure of volatility and potential support/resistance levels.
 * @param data - Array of data points to analyze.
 * @param period - Number of periods for the moving average.
 * @param standardDeviations - Number of standard deviations for the bands.
 * @returns Bollinger Bands data with upper, middle, and lower bands.
 */
export function calculateBollingerBands(
  data: DataPoint[],
  period = 20,
  standardDeviations = 2
): BollingerBands[] {
  if (data.length < period) return [];

  const bands: BollingerBands[] = [];

  for (let i = period - 1; i < data.length; i++) {
    const slice = data.slice(i - period + 1, i + 1);

    // Calculate middle band (SMA)
    const middle = slice.reduce((sum, point) => sum + point.value, 0) / period;

    // Calculate standard deviation
    const variance =
      slice.reduce((sum, point) => sum + Math.pow(point.value - middle, 2), 0) / period;
    const stdDev = Math.sqrt(variance);

    bands.push({
      date: data[i].date,
      upper: middle + standardDeviations * stdDev,
      middle: middle,
      lower: middle - standardDeviations * stdDev,
    });
  }

  return bands;
}

/**
 * Calculate Relative Strength Index (RSI)
 * Momentum oscillator measuring the speed and magnitude of price changes.
 * @param data - Array of data points to analyze.
 * @param period - Number of periods for the RSI calculation.
 * @returns Array of RSI values.
 */
export function calculateRSI(data: DataPoint[], period = 14): RSIPoint[] {
  if (data.length < period + 1) return [];

  const rsiPoints: RSIPoint[] = [];
  const gains: number[] = [];
  const losses: number[] = [];

  // Calculate initial gains and losses
  for (let i = 1; i < data.length; i++) {
    const change = data[i].value - data[i - 1].value;
    gains.push(change > 0 ? change : 0);
    losses.push(change < 0 ? Math.abs(change) : 0);
  }

  // Calculate initial average gain and loss
  let avgGain = gains.slice(0, period).reduce((sum, gain) => sum + gain, 0) / period;
  let avgLoss = losses.slice(0, period).reduce((sum, loss) => sum + loss, 0) / period;

  // Calculate RSI for each point
  for (let i = period; i < data.length; i++) {
    const rs = avgGain / (avgLoss || 0.001); // Avoid division by zero
    const rsi = 100 - 100 / (1 + rs);

    rsiPoints.push({
      date: data[i].date,
      rsi: rsi,
    });

    // Update averages using Wilder's smoothing
    if (i < gains.length) {
      avgGain = (avgGain * (period - 1) + gains[i]) / period;
      avgLoss = (avgLoss * (period - 1) + losses[i]) / period;
    }
  }

  return rsiPoints;
}

/**
 * Calculate Rate of Change (ROC)
 * Momentum indicator showing percentage change over a specific period.
 * @param data - Array of data points to analyze.
 * @param period - Number of periods for the RSI calculation.
 * @returns Array of ROC values.
 */
export function calculateROC(data: DataPoint[], period: number): TechnicalIndicator[] {
  if (data.length < period + 1) return [];

  const roc: TechnicalIndicator[] = [];

  for (let i = period; i < data.length; i++) {
    const currentValue = data[i].value;
    const previousValue = data[i - period].value;
    const rocValue = ((currentValue - previousValue) / previousValue) * 100;

    roc.push({
      date: data[i].date,
      value: rocValue,
      indicator: `ROC(${period})`,
    });
  }

  return roc;
}

/**
 * Calculate Standard Deviation
 * Measure of volatility and dispersion.
 * @param data - Array of data points to analyze.
 * @param period - Number of periods for the RSI calculation.
 * @returns Array of standard deviation values.
 */
export function calculateStandardDeviation(
  data: DataPoint[],
  period: number
): TechnicalIndicator[] {
  if (data.length < period) return [];

  const stdDevs: TechnicalIndicator[] = [];

  for (let i = period - 1; i < data.length; i++) {
    const slice = data.slice(i - period + 1, i + 1);
    const mean = slice.reduce((sum, point) => sum + point.value, 0) / period;
    const variance =
      slice.reduce((sum, point) => sum + Math.pow(point.value - mean, 2), 0) / period;
    const stdDev = Math.sqrt(variance);

    stdDevs.push({
      date: data[i].date,
      value: stdDev,
      indicator: `StdDev(${period})`,
    });
  }

  return stdDevs;
}

/**
 * Detect Economic Cycles
 * Identify potential turning points and trend changes.
 */
export interface CyclePoint {
  date: string;
  type: 'peak' | 'trough' | 'expansion' | 'contraction';
  value: number;
  confidence: number;
}

/**
 * Detect economic cycles in time series data.
 * Identifies peaks and troughs in economic indicators.
 * @param data - Array of data points to analyze.
 * @param lookback - Number of periods to look back for the calculation.
 * @returns Array of cycle points with peaks and troughs.
 */
export function detectEconomicCycles(data: DataPoint[], lookback = 6): CyclePoint[] {
  if (data.length < lookback * 2 + 1) return [];

  const cycles: CyclePoint[] = [];

  for (let i = lookback; i < data.length - lookback; i++) {
    const current = data[i];
    const before = data.slice(i - lookback, i);
    const after = data.slice(i + 1, i + lookback + 1);

    const beforeMax = Math.max(...before.map(p => p.value));
    const beforeMin = Math.min(...before.map(p => p.value));
    const afterMax = Math.max(...after.map(p => p.value));
    const afterMin = Math.min(...after.map(p => p.value));

    // Detect peaks
    if (current.value > beforeMax && current.value > afterMax) {
      const confidence = Math.min(
        (current.value - beforeMax) / beforeMax,
        (current.value - afterMax) / afterMax
      );

      cycles.push({
        date: current.date,
        type: 'peak',
        value: current.value,
        confidence: Math.max(0, Math.min(1, confidence)),
      });
    }

    // Detect troughs
    if (current.value < beforeMin && current.value < afterMin) {
      const confidence = Math.min(
        (beforeMin - current.value) / beforeMin,
        (afterMin - current.value) / afterMin
      );

      cycles.push({
        date: current.date,
        type: 'trough',
        value: current.value,
        confidence: Math.max(0, Math.min(1, confidence)),
      });
    }
  }

  return cycles;
}

/**
 * Calculate correlation between two economic series.
 * @param series1 - First data series for comparison.
 * @param series2 - Second data series for comparison.
 * @returns Correlation coefficient between the two series.
 */
export function calculateCorrelation(series1: DataPoint[], series2: DataPoint[]): number {
  if (series1.length !== series2.length || series1.length === 0) return 0;

  const n = series1.length;
  const sum1 = series1.reduce((sum, point) => sum + point.value, 0);
  const sum2 = series2.reduce((sum, point) => sum + point.value, 0);
  const sum1Sq = series1.reduce((sum, point) => sum + point.value * point.value, 0);
  const sum2Sq = series2.reduce((sum, point) => sum + point.value * point.value, 0);
  const sumProducts = series1.reduce((sum, point, i) => sum + point.value * series2[i].value, 0);

  const numerator = n * sumProducts - sum1 * sum2;
  const denominator = Math.sqrt((n * sum1Sq - sum1 * sum1) * (n * sum2Sq - sum2 * sum2));

  return denominator === 0 ? 0 : numerator / denominator;
}

/**
 * Economic Event Types for Chart Annotations.
 */
export interface EconomicEvent {
  date: string;
  title: string;
  description: string;
  type: 'recession' | 'expansion' | 'policy' | 'crisis' | 'announcement';
  impact: 'high' | 'medium' | 'low';
  source: string;
}

/**
 * Historical Economic Events Database
 * Major events that should be annotated on economic charts.
 */
export const MAJOR_ECONOMIC_EVENTS: EconomicEvent[] = [
  {
    date: '2020-03-01',
    title: 'COVID-19 Pandemic Begins',
    description: 'WHO declares COVID-19 a pandemic, triggering global economic shutdown',
    type: 'crisis',
    impact: 'high',
    source: 'WHO',
  },
  {
    date: '2020-03-15',
    title: 'Fed Emergency Rate Cut',
    description: 'Federal Reserve cuts interest rates to near zero in emergency meeting',
    type: 'policy',
    impact: 'high',
    source: 'Federal Reserve',
  },
  {
    date: '2008-09-15',
    title: 'Lehman Brothers Collapse',
    description:
      'Investment bank Lehman Brothers files for bankruptcy, triggering financial crisis',
    type: 'crisis',
    impact: 'high',
    source: 'Financial Markets',
  },
  {
    date: '2008-12-01',
    title: 'Great Recession Begins',
    description: 'NBER officially dates the beginning of the Great Recession',
    type: 'recession',
    impact: 'high',
    source: 'NBER',
  },
  {
    date: '2009-06-01',
    title: 'Great Recession Ends',
    description: 'NBER officially dates the end of the Great Recession',
    type: 'expansion',
    impact: 'high',
    source: 'NBER',
  },
  {
    date: '2001-03-01',
    title: 'Dot-com Recession Begins',
    description: 'Technology bubble bursts, leading to economic recession',
    type: 'recession',
    impact: 'medium',
    source: 'NBER',
  },
  {
    date: '2001-11-01',
    title: 'Dot-com Recession Ends',
    description: 'Economic recovery begins following dot-com crash',
    type: 'expansion',
    impact: 'medium',
    source: 'NBER',
  },
];

/**
 * Get relevant economic events for a date range.
 * @param startDate - Start date for the period.
 * @param endDate - End date for the period.
 * @returns Array of economic events within the date range.
 */
export function getEconomicEventsInRange(startDate: string, endDate: string): EconomicEvent[] {
  const start = new Date(startDate);
  const end = new Date(endDate);

  return MAJOR_ECONOMIC_EVENTS.filter(event => {
    const eventDate = new Date(event.date);
    return eventDate >= start && eventDate <= end;
  });
}
