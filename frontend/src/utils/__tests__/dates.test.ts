import { formatIsoDate, parseIsoDate } from '../dates';

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
