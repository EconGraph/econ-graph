import {
  annotationInRange,
  buildAnnotationOptions,
  selectShownPoints,
  visibleRange,
} from '../seriesChartData';
import { parseIsoDate } from '../../../utils/dates';

const range = (start: string | null, end: string | null) => ({
  start: start ? parseIsoDate(start) : null,
  end: end ? parseIsoDate(end) : null,
});

describe('selectShownPoints', () => {
  const points = [
    { date: '2024-03-01', value: 3 },
    { date: '2024-01-01', value: 1 },
    { date: 'not a date', value: 9 },
    { date: '2024-02-01', value: 2 },
  ];

  test('sorts by date and drops unparseable dates', () => {
    expect(selectShownPoints(points).map(p => p.value)).toEqual([1, 2, 3]);
  });

  test('keeps both inclusive bounds', () => {
    expect(
      selectShownPoints(points, range('2024-02-01', '2024-03-01')).map(p => p.value)
    ).toEqual([2, 3]);
  });

  test('treats an invalid (half-typed) bound as open', () => {
    expect(
      selectShownPoints(points, { start: new Date(NaN), end: parseIsoDate('2024-01-01') }).map(
        p => p.value
      )
    ).toEqual([1]);
  });
});

describe('annotationInRange', () => {
  const point = { kind: 'point' as const, id: 'p', label: 'P', date: '2024-02-01' };
  const span = {
    kind: 'range' as const,
    id: 'r',
    label: 'R',
    startDate: '2024-01-01',
    endDate: '2024-03-01',
  };

  test('a point is in range when its date is', () => {
    expect(annotationInRange(point, range('2024-02-01', '2024-02-01'))).toBe(true);
    expect(annotationInRange(point, range('2024-02-02', null))).toBe(false);
  });

  test('a range is in range when it overlaps', () => {
    expect(annotationInRange(span, range('2024-02-15', '2024-06-01'))).toBe(true);
    expect(annotationInRange(span, range(null, '2024-01-01'))).toBe(true);
    expect(annotationInRange(span, range('2024-03-02', null))).toBe(false);
  });

  test('an open range includes any valid annotation', () => {
    expect(annotationInRange(point)).toBe(true);
    expect(annotationInRange(span, range(null, null))).toBe(true);
  });

  test('an annotation with a bad date or a reversed range is never drawn', () => {
    expect(annotationInRange({ ...point, date: '' }, range(null, null))).toBe(false);
    expect(
      annotationInRange({ ...span, startDate: '2024-03-01', endDate: '2024-01-01' })
    ).toBe(false);
  });
});

describe('buildAnnotationOptions', () => {
  test('uses the default colour and a translucent fill for ranges', () => {
    const options = buildAnnotationOptions(
      [{ kind: 'range', id: 'r', label: 'R', startDate: '2024-01-01', endDate: '2024-02-01' }],
      range(null, null),
      '#ed6c02'
    );
    expect(options.r).toMatchObject({
      type: 'box',
      borderColor: '#ed6c02',
      backgroundColor: 'rgba(237, 108, 2, 0.15)',
      drawTime: 'beforeDatasetsDraw',
    });
    expect((options.r as { click?: unknown }).click).toBeUndefined();
  });

  test('shades named colours and leaves unparseable ones transparent', () => {
    const options = buildAnnotationOptions(
      [
        { kind: 'range', id: 'named', label: 'N', startDate: '2024-01-01', endDate: '2024-01-01', color: 'red' },
        { kind: 'range', id: 'bad', label: 'B', startDate: '2024-01-01', endDate: '2024-01-01', color: '#zz' },
      ],
      range(null, null),
      '#ed6c02'
    );
    expect(options.named).toMatchObject({ backgroundColor: 'rgba(255, 0, 0, 0.15)' });
    expect(options.bad).toMatchObject({ backgroundColor: 'transparent' });
  });

  test('a one-day range covers the whole day', () => {
    const options = buildAnnotationOptions(
      [{ kind: 'range', id: 'd', label: 'D', startDate: '2024-03-10', endDate: '2024-03-10' }],
      range(null, null),
      '#ed6c02'
    );
    expect(options.d).toMatchObject({
      xMin: parseIsoDate('2024-03-10').getTime(),
      xMax: parseIsoDate('2024-03-11').getTime(),
    });
  });
});

describe('visibleRange', () => {
  const shown = [
    { date: '2024-01-01', value: 1 },
    { date: '2024-03-01', value: 3 },
  ];

  test('fills open bounds from the first and last shown points', () => {
    expect(visibleRange(range(null, null), shown)).toEqual(range('2024-01-01', '2024-03-01'));
    expect(visibleRange(range('2023-06-01', null), shown)).toEqual(
      range('2023-06-01', '2024-03-01')
    );
  });

  test('stays open without points', () => {
    expect(visibleRange(range(null, null), [])).toEqual(range(null, null));
  });
});

describe('buildAnnotationOptions clipping', () => {
  test('skips a range that only starts on the last visible day', () => {
    const options = buildAnnotationOptions(
      [
        { kind: 'range', id: 'edge', label: 'E', startDate: '2024-03-01', endDate: '2024-06-01' },
        { kind: 'point', id: 'last', label: 'L', date: '2024-03-01' },
      ],
      range('2024-01-01', '2024-03-01'),
      '#ed6c02'
    );
    expect(Object.keys(options)).toEqual(['last']);
  });


  test('cuts a range box to the visible range and skips annotations outside it', () => {
    const options = buildAnnotationOptions(
      [
        { kind: 'range', id: 'long', label: 'L', startDate: '1929-08-01', endDate: '2030-01-01' },
        { kind: 'point', id: 'early', label: 'E', date: '1990-01-01' },
      ],
      range('2024-01-01', '2024-03-01'),
      '#ed6c02'
    );
    expect(Object.keys(options)).toEqual(['long']);
    expect(options.long).toMatchObject({
      xMin: parseIsoDate('2024-01-01').getTime(),
      xMax: parseIsoDate('2024-03-01').getTime(),
    });
  });
});
