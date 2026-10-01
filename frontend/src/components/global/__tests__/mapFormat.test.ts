import { describe, expect, it } from 'vitest';
import { formatMapDate, formatMapValue } from '../mapFormat';

describe('formatMapValue', () => {
  it('groups thousands and keeps at most two decimals, without trailing zeros', () => {
    expect(formatMapValue('82769.4')).toBe('82,769.4');
    expect(formatMapValue('334914895')).toBe('334,914,895');
    expect(formatMapValue('0.2')).toBe('0.2');
    expect(formatMapValue('3.14159')).toBe('3.14');
    expect(formatMapValue('5.000')).toBe('5');
    expect(formatMapValue('-1.50')).toBe('-1.5');
  });

  it('passes through what is not a decimal', () => {
    expect(formatMapValue('n/a')).toBe('n/a');
  });
});

describe('formatMapDate', () => {
  it('shows the year alone for annual series', () => {
    expect(formatMapDate('2023-01-01', 'Annual')).toBe('2023');
  });

  it('shows the quarter or month for quarterly and monthly series', () => {
    expect(formatMapDate('2024-04-01', 'Quarterly')).toBe('2024 Q2');
    expect(formatMapDate('2024-12-01', 'Quarterly')).toBe('2024 Q4');
    expect(formatMapDate('2024-03-01', 'Monthly')).toBe('Mar 2024');
  });

  it('shows the full date otherwise', () => {
    expect(formatMapDate('2024-03-15', 'Daily')).toBe('Mar 15, 2024');
    expect(formatMapDate('2024-03-01', null)).toBe('Mar 1, 2024');
  });
});
