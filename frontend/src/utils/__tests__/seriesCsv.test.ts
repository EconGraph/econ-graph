import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  REVOKE_DELAY_MS,
  buildSeriesCsv,
  csvField,
  csvTextField,
  downloadCsv,
  seriesCsvFileName,
} from '../seriesCsv';

const retrievedAt = new Date(Date.UTC(2026, 8, 26, 12, 30, 0));

const base = {
  title: 'Real Gross Domestic Product',
  source: 'FRED',
  units: 'Billions of Chained 2017 Dollars',
  transformation: '',
  retrievedAt,
  points: [] as { date: string; value: number | null }[],
};

describe('csvField', () => {
  it('leaves plain text alone', () => {
    expect(csvField('GDP')).toBe('GDP');
  });

  it('quotes commas, quotes and line breaks, doubling inner quotes', () => {
    expect(csvField('Prices, all items')).toBe('"Prices, all items"');
    expect(csvField('The "core" rate')).toBe('"The ""core"" rate"');
    expect(csvField('line one\nline two')).toBe('"line one\nline two"');
    expect(csvField('a\r\nb')).toBe('"a\r\nb"');
  });
});

describe('csvTextField', () => {
  it('stops text from starting a spreadsheet formula', () => {
    expect(csvTextField('=HYPERLINK("x")')).toBe(`"'=HYPERLINK(""x"")"`);
    expect(csvTextField('+1')).toBe("'+1");
    expect(csvTextField('-1')).toBe("'-1");
    expect(csvTextField('@SUM(A1)')).toBe("'@SUM(A1)");
  });

  it('leaves ordinary text alone', () => {
    expect(csvTextField('Index 1982-1984=100')).toBe('Index 1982-1984=100');
  });
});

describe('buildSeriesCsv', () => {
  it('writes metadata rows, then date,value rows with ISO dates, CRLF line ends', () => {
    const csv = buildSeriesCsv({
      ...base,
      transformation: 'Year-over-Year % Change',
      points: [
        { date: '2024-01-01', value: 2.5 },
        { date: '2024-04-01', value: -0.125 },
      ],
    });
    expect(csv).toBe(
      [
        'Title,Real Gross Domestic Product',
        'Source,FRED',
        'Units,Billions of Chained 2017 Dollars',
        'Transformation,Year-over-Year % Change',
        'Retrieved at,2026-09-26T12:30:00.000Z',
        'date,value',
        '2024-01-01,2.5',
        '2024-04-01,-0.125',
        '',
      ].join('\r\n')
    );
  });

  it('escapes commas and quotes in the title', () => {
    const csv = buildSeriesCsv({ ...base, title: 'Consumer Price Index, "All Items"' });
    expect(csv.split('\r\n')[0]).toBe('Title,"Consumer Price Index, ""All Items"""');
  });

  it('writes the header only for an empty series, and None for levels', () => {
    const lines = buildSeriesCsv({ ...base, source: '', units: '' }).split('\r\n');
    expect(lines).toEqual([
      'Title,Real Gross Domestic Product',
      'Source,',
      'Units,',
      'Transformation,None',
      'Retrieved at,2026-09-26T12:30:00.000Z',
      'date,value',
      '',
    ]);
  });

  it('writes only the calendar date when a date carries a time', () => {
    const csv = buildSeriesCsv({ ...base, points: [{ date: '2024-01-01T00:00:00Z', value: 1 }] });
    expect(csv.split('\r\n')[6]).toBe('2024-01-01,1');
  });

  it('leaves the retrieval time empty when unknown', () => {
    const csv = buildSeriesCsv({ ...base, retrievedAt: null });
    expect(csv.split('\r\n')[4]).toBe('Retrieved at,');
  });

  it('writes a missing value as an empty field', () => {
    const csv = buildSeriesCsv({ ...base, points: [{ date: '2024-01-01', value: null }] });
    expect(csv.split('\r\n')[6]).toBe('2024-01-01,');
  });
});

describe('seriesCsvFileName', () => {
  it('names levels after the series and adds the transformation otherwise', () => {
    expect(seriesCsvFileName('GDPC1', 'NONE')).toBe('GDPC1.csv');
    expect(seriesCsvFileName('GDPC1', 'YEAR_OVER_YEAR')).toBe('GDPC1-year_over_year.csv');
  });

  it('replaces characters that are unsafe in file names', () => {
    expect(seriesCsvFileName('CES/0000:0001', 'NONE')).toBe('CES_0000_0001.csv');
    expect(seriesCsvFileName('../..', 'NONE')).toBe('series.csv');
  });
});

describe('downloadCsv', () => {
  const originalCreate = URL.createObjectURL;
  const originalRevoke = URL.revokeObjectURL;

  afterEach(() => {
    URL.createObjectURL = originalCreate;
    URL.revokeObjectURL = originalRevoke;
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('saves a UTF-8 file with a byte-order mark, then cleans up', async () => {
    vi.useFakeTimers();
    let blob: Blob | undefined;
    URL.createObjectURL = vi.fn((b: Blob) => {
      blob = b;
      return 'blob:csv';
    });
    URL.revokeObjectURL = vi.fn();
    const click = vi
      .spyOn(window.HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => undefined);

    downloadCsv('a.csv', 'x,y\r\n');

    expect(click).toHaveBeenCalledTimes(1);
    expect(document.querySelector('a[download]')).toBeNull();
    expect(blob?.type).toBe('text/csv;charset=utf-8');
    const bytes = new Uint8Array(await blob!.arrayBuffer());
    expect(Array.from(bytes.slice(0, 3))).toEqual([0xef, 0xbb, 0xbf]);
    expect(new window.TextDecoder().decode(bytes.slice(3))).toBe('x,y\r\n');

    expect(URL.revokeObjectURL).not.toHaveBeenCalled();
    vi.advanceTimersByTime(REVOKE_DELAY_MS);
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:csv');
  });
});
