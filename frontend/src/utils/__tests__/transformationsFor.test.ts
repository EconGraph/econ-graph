/**
 * REQUIREMENT: The series page offers only transformations a series' frequency supports, and
 * shows small transformed values with enough precision to read.
 * PURPOSE: Regression tests for ECO-407 (month-over-month offered on quarterly series; log
 * differences shown as 0.01).
 */

import { formatTransformedValue, transformationsFor } from '../transformations';

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
