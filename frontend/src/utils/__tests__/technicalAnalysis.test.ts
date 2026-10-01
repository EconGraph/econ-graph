import { describe, expect, it } from 'vitest';
import {
  type DataPoint,
  calculateSMA,
  calculateEMA,
  calculateBollingerBands,
  calculateRSI,
  calculateROC,
  calculateStandardDeviation,
  detectEconomicCycles,
  calculateCorrelation,
  getEconomicEventsInRange,
} from '../technicalAnalysis';

const points = (values: number[]): DataPoint[] =>
  values.map((value, i) => ({ date: `2024-01-${String(i + 1).padStart(2, '0')}`, value }));

describe('calculateSMA', () => {
  it('averages each window of `period` points', () => {
    expect(calculateSMA(points([1, 2, 3, 4, 5]), 3)).toEqual([
      { date: '2024-01-03', value: 2, indicator: 'SMA(3)' },
      { date: '2024-01-04', value: 3, indicator: 'SMA(3)' },
      { date: '2024-01-05', value: 4, indicator: 'SMA(3)' },
    ]);
  });

  it('returns nothing when there are fewer points than the period', () => {
    expect(calculateSMA(points([1, 2]), 3)).toEqual([]);
  });
});

describe('calculateEMA', () => {
  it('seeds with the simple average of the first `period` points, then weights recent points more heavily', () => {
    expect(calculateEMA(points([1, 2, 3, 4]), 2)).toEqual([
      { date: '2024-01-02', value: 1.5, indicator: 'EMA(2)' },
      { date: '2024-01-03', value: 2.5, indicator: 'EMA(2)' },
      { date: '2024-01-04', value: 3.5, indicator: 'EMA(2)' },
    ]);
  });

  it('returns nothing when there are fewer points than the period', () => {
    expect(calculateEMA(points([1]), 2)).toEqual([]);
  });
});

describe('calculateBollingerBands', () => {
  it('centers the middle band on the moving average, offset by the standard deviation', () => {
    expect(calculateBollingerBands(points([2, 4, 6, 8]), 2, 1)).toEqual([
      { date: '2024-01-02', upper: 4, middle: 3, lower: 2 },
      { date: '2024-01-03', upper: 6, middle: 5, lower: 4 },
      { date: '2024-01-04', upper: 8, middle: 7, lower: 6 },
    ]);
  });

  it('returns nothing when there are fewer points than the period', () => {
    expect(calculateBollingerBands(points([1, 2]), 5)).toEqual([]);
  });

  it('defaults to a 20-period window and 2 standard deviations', () => {
    const result = calculateBollingerBands(points(Array(20).fill(5)));
    expect(result).toEqual([{ date: '2024-01-20', upper: 5, middle: 5, lower: 5 }]);
  });
});

describe('calculateRSI', () => {
  it('is 100 for a strictly increasing series (no losses)', () => {
    const result = calculateRSI(points([1, 2, 3, 4, 5, 6]), 3);
    expect(result).toHaveLength(3);
    result.forEach(point => expect(point.rsi).toBe(100));
  });

  it('is 0 for a strictly decreasing series', () => {
    const result = calculateRSI(points([6, 5, 4, 3, 2, 1]), 3);
    expect(result).toHaveLength(3);
    result.forEach(point => expect(point.rsi).toBe(0));
  });

  it('returns nothing when there are fewer points than period + 1', () => {
    expect(calculateRSI(points([1, 2, 3]), 5)).toEqual([]);
  });

  it('defaults to a 14-period window', () => {
    const result = calculateRSI(points(Array.from({ length: 15 }, (_, i) => i + 1)));
    expect(result).toEqual([{ date: '2024-01-15', rsi: 100 }]);
  });
});

describe('calculateROC', () => {
  it('computes percentage change against the point `period` steps back', () => {
    expect(calculateROC(points([10, 20, 15, 30]), 2)).toEqual([
      { date: '2024-01-03', value: 50, indicator: 'ROC(2)' },
      { date: '2024-01-04', value: 50, indicator: 'ROC(2)' },
    ]);
  });

  it('returns nothing when there are fewer points than period + 1', () => {
    expect(calculateROC(points([1, 2]), 5)).toEqual([]);
  });

  it('is Infinity when the earlier value is zero', () => {
    expect(calculateROC(points([0, 5]), 1)[0].value).toBe(Infinity);
  });
});

describe('calculateStandardDeviation', () => {
  it('computes the population standard deviation of each window', () => {
    const result = calculateStandardDeviation(points([2, 4, 6, 8]), 2);
    expect(result).toEqual([
      { date: '2024-01-02', value: 1, indicator: 'StdDev(2)' },
      { date: '2024-01-03', value: 1, indicator: 'StdDev(2)' },
      { date: '2024-01-04', value: 1, indicator: 'StdDev(2)' },
    ]);
  });

  it('returns nothing when there are fewer points than the period', () => {
    expect(calculateStandardDeviation(points([1]), 2)).toEqual([]);
  });
});

describe('detectEconomicCycles', () => {
  it('flags a point higher than everything around it as a peak', () => {
    const result = detectEconomicCycles(points([1, 5, 1]), 1);
    expect(result).toEqual([{ date: '2024-01-02', type: 'peak', value: 5, confidence: 1 }]);
  });

  it('scales peak confidence by how far the point clears its neighbors, clamped to [0, 1]', () => {
    const result = detectEconomicCycles(points([9, 10, 9]), 1);
    expect(result).toEqual([
      { date: '2024-01-02', type: 'peak', value: 10, confidence: expect.closeTo(1 / 9, 10) },
    ]);
  });

  it('flags a point lower than everything around it as a trough', () => {
    const result = detectEconomicCycles(points([5, 1, 5]), 1);
    expect(result).toEqual([{ date: '2024-01-02', type: 'trough', value: 1, confidence: 0.8 }]);
  });

  it('flags nothing for a flat series', () => {
    expect(detectEconomicCycles(points([5, 5, 5]), 1)).toEqual([]);
  });

  it('returns nothing when there are fewer than 2 * lookback + 1 points', () => {
    expect(detectEconomicCycles(points([1, 2]), 1)).toEqual([]);
  });
});

describe('calculateCorrelation', () => {
  it('is 1 for perfectly correlated series', () => {
    expect(calculateCorrelation(points([1, 2, 3, 4]), points([2, 4, 6, 8]))).toBeCloseTo(1, 10);
  });

  it('is -1 for perfectly anti-correlated series', () => {
    expect(calculateCorrelation(points([1, 2, 3, 4]), points([8, 6, 4, 2]))).toBeCloseTo(-1, 10);
  });

  it('is 0 when the series have different lengths', () => {
    expect(calculateCorrelation(points([1, 2, 3]), points([1, 2]))).toBe(0);
  });

  it('is 0 for empty series', () => {
    expect(calculateCorrelation([], [])).toBe(0);
  });
});

describe('getEconomicEventsInRange', () => {
  it('returns events that fall within the date range, inclusive', () => {
    const events = getEconomicEventsInRange('2020-03-01', '2020-03-15');
    expect(events.map(e => e.title)).toEqual([
      'COVID-19 Pandemic Begins',
      'Fed Emergency Rate Cut',
    ]);
  });

  it('returns nothing for a range with no events', () => {
    expect(getEconomicEventsInRange('1990-01-01', '1990-12-31')).toEqual([]);
  });
});
