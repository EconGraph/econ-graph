import { parseDashboardSeries } from '../dashboardSeries';

const valid = {
  id: 'gdp',
  label: 'Gross Domestic Product',
  sourceName: 'Federal Reserve Economic Data (FRED)',
  sourceLabel: 'FRED',
  externalId: 'GDP',
  fractionDigits: 1,
};

describe('parseDashboardSeries', () => {
  test('accepts well-formed entries', () => {
    expect(parseDashboardSeries([valid])).toEqual([valid]);
  });

  test('rejects a file that is not an array', () => {
    expect(() => parseDashboardSeries({})).toThrow('must be an array');
  });

  test.each(['id', 'label', 'sourceName', 'sourceLabel', 'externalId'])(
    'rejects an entry without %s',
    field => {
      expect(() => parseDashboardSeries([{ ...valid, [field]: '' }])).toThrow(`"${field}"`);
    }
  );

  test.each([undefined, -1, 1.5, 11, '2'])('rejects fractionDigits %j', fractionDigits => {
    expect(() => parseDashboardSeries([{ ...valid, fractionDigits }])).toThrow('fractionDigits');
  });

  test('rejects duplicate ids', () => {
    expect(() => parseDashboardSeries([valid, { ...valid }])).toThrow('more than once');
  });
});
