/**
 * REQUIREMENT: The series page offers only transformations a series' frequency supports, and
 * shows small transformed values with enough precision to read.
 * PURPOSE: Regression tests for ECO-407 (month-over-month offered on quarterly series; log
 * differences shown as 0.01).
 */

import {
  formatTransformedValue,
  levelFractionDigits,
  scaledTickLabel,
  transformationsFor,
} from '../transformations';

const offered = (frequency: string) => transformationsFor(frequency).map(o => o.value);

describe('transformationsFor', () => {
  it('offers every transformation for monthly and finer series', () => {
    for (const frequency of ['Monthly', 'Weekly, Ending Friday', 'Daily', 'Daily, Close']) {
      expect(offered(frequency)).toContain('MONTH_OVER_MONTH');
      expect(offered(frequency)).toContain('QUARTER_OVER_QUARTER');
    }
  });

  it('drops month-over-month for quarterly series', () => {
    expect(offered('Quarterly')).not.toContain('MONTH_OVER_MONTH');
    expect(offered('Quarterly')).toContain('QUARTER_OVER_QUARTER');
    expect(offered('Quarterly')).toContain('LOG_DIFFERENCE');
  });

  it('keeps only year-over-year among period changes for semi-annual and annual series', () => {
    for (const frequency of ['Semi-Annual', 'Semiannual', 'Annual', 'annual']) {
      expect(offered(frequency)).toEqual([
        'NONE',
        'YEAR_OVER_YEAR',
        'PERCENT_CHANGE',
        'LOG_DIFFERENCE',
      ]);
    }
  });

  it('drops quarter-over-quarter for biweekly series, whose points miss its window', () => {
    expect(offered('Biweekly, Ending Wednesday')).toContain('MONTH_OVER_MONTH');
    expect(offered('Biweekly, Ending Wednesday')).not.toContain('QUARTER_OVER_QUARTER');
  });

  it('offers everything for a frequency it does not recognise', () => {
    expect(offered('Irregular')).toHaveLength(6);
  });
});

describe('formatTransformedValue', () => {
  it('keeps four significant digits for log differences', () => {
    expect(formatTransformedValue(0.011879, 'LOG_DIFFERENCE')).toBe('0.01188');
    expect(formatTransformedValue(-0.000523, 'LOG_DIFFERENCE')).toBe('-0.0005230');
    // The same number of digits down the column
    expect(formatTransformedValue(0.01, 'LOG_DIFFERENCE')).toBe('0.01000');
  });

  it('shows two decimals with grouped thousands otherwise', () => {
    expect(formatTransformedValue(5.3, 'YEAR_OVER_YEAR')).toBe('5.30');
    expect(formatTransformedValue(31702.1, 'NONE')).toBe('31,702.10');
  });
});

// ECO-415: integer counts showed .00 (7,324,017.00 establishments) and BEA GDP's y-axis wrote out
// 14-digit dollar amounts.
describe("levels at their series' precision", () => {
  it('needs no decimals for counts and whole-dollar levels', () => {
    expect(levelFractionDigits([7106316, null, 7324017])).toBe(0);
    expect(levelFractionDigits([28_624_702_000_000])).toBe(0);
  });

  it('keeps the decimals the most precise value carries, up to two', () => {
    expect(levelFractionDigits([3.7, 4, 3.8])).toBe(1);
    expect(levelFractionDigits([32101.6, 31722.514])).toBe(2);
    expect(levelFractionDigits([])).toBe(0);
  });

  it('formats a level with the given decimals and a percent change with two', () => {
    expect(formatTransformedValue(7324017, 'NONE', 0)).toBe('7,324,017');
    expect(formatTransformedValue(3.8, 'NONE', 1)).toBe('3.8');
    expect(formatTransformedValue(1.2, 'QUARTER_OVER_QUARTER', 0)).toBe('1.20');
    expect(formatTransformedValue(0.011879, 'LOG_DIFFERENCE', 0)).toBe('0.01188');
  });
});

describe('scaledTickLabel', () => {
  const labels = (ticks: number[]) => ticks.map(t => scaledTickLabel(t, ticks));

  it('scales trillions, billions and millions to the largest tick', () => {
    expect(labels([0, 10e12, 20e12, 30e12])).toEqual(['0T', '10T', '20T', '30T']);
    expect(labels([1.5e9, 2e9, 2.5e9])).toEqual(['1.5B', '2.0B', '2.5B']);
    expect(labels([0, 500_000, 1_000_000])).toEqual(['0.0M', '0.5M', '1.0M']);
  });

  it('keeps enough decimals that neighbouring ticks differ', () => {
    const ticks = [28.6245e12, 28.62455e12, 28.6246e12];
    const shown = labels(ticks);
    expect(new Set(shown).size).toBe(3);
    expect(shown[0]).toBe('28.62450T');
  });

  it('leaves values under a million to Chart.js', () => {
    expect(scaledTickLabel(300, [100, 200, 300])).toBeUndefined();
  });
});
