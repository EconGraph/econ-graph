import { formatIsoDate, formatObservationDate, parseIsoDate } from '../dates';

// West of UTC, reading '2024-01-01' as UTC gives Dec 31; pin a zone where that shows.
const originalTz = process.env.TZ;
beforeAll(() => {
  process.env.TZ = 'America/Los_Angeles';
});
afterAll(() => {
  if (originalTz === undefined) delete process.env.TZ;
  else process.env.TZ = originalTz;
});

describe('parseIsoDate', () => {
  test('reads a calendar date as local midnight', () => {
    const date = parseIsoDate('2024-03-01');
    expect([date.getFullYear(), date.getMonth(), date.getDate(), date.getHours()]).toEqual([
      2024, 2, 1, 0,
    ]);
  });

  test('ignores a time part', () => {
    expect(parseIsoDate('2024-03-01T23:30:00Z').getDate()).toBe(1);
  });

  test('returns an invalid date for other strings', () => {
    expect(Number.isNaN(parseIsoDate('March 2024').getTime())).toBe(true);
  });
});

describe('formatIsoDate', () => {
  test('formats the calendar date, not the UTC instant', () => {
    expect(formatIsoDate('2024-01-01')).toBe('Jan 1, 2024');
  });
});

// ECO-415: quarterly and annual observations read as the first day of their period.
describe('formatObservationDate', () => {
  test('names the quarter of a quarterly observation', () => {
    expect(formatObservationDate('2026-04-01', 'Quarterly')).toBe('2026 Q2');
    expect(formatObservationDate('2026-01-01', 'Quarterly')).toBe('2026 Q1');
    // Dated at the quarter's end, it is still that quarter.
    expect(formatObservationDate('2025-12-31', 'Quarterly, End of Period')).toBe('2025 Q4');
  });

  test('names the half of a semi-annual observation', () => {
    expect(formatObservationDate('2024-01-01', 'Semiannual')).toBe('2024 H1');
    expect(formatObservationDate('2024-07-01', 'Semi-annual')).toBe('2024 H2');
  });

  test('names the year of an annual observation, in any timezone', () => {
    expect(formatObservationDate('2022-01-01', 'Annual')).toBe('2022');
    expect(formatObservationDate('2022-01-01', 'Yearly')).toBe('2022');
  });

  test('shows the calendar date for other frequencies', () => {
    expect(formatObservationDate('2024-03-01', 'Monthly')).toBe('Mar 1, 2024');
    expect(formatObservationDate('2024-01-05', 'Weekly, Ending Friday')).toBe('Jan 5, 2024');
    expect(formatObservationDate('2024-01-05', 'Irregular')).toBe('Jan 5, 2024');
    expect(
      formatObservationDate('2024-03-01', 'Daily', { year: 'numeric', month: 'long', day: 'numeric' })
    ).toBe('March 1, 2024');
  });
});
