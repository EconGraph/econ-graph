import React from 'react';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { vi } from 'vitest';
import { Line } from 'react-chartjs-2';
import SeriesChart, { SeriesChartAnnotation, SeriesChartProps } from '../SeriesChart';
import { parseIsoDate } from '../../../utils/dates';

vi.mock('react-chartjs-2', () => ({
  Line: vi.fn(() => <div data-testid='line-chart' />),
}));

const lineMock = vi.mocked(Line);

/** The props the chart last passed to react-chartjs-2's Line. */
function lastLineProps(): { data: any; options: any } {
  const calls = lineMock.mock.calls;
  return calls[calls.length - 1][0] as any;
}

const at = (iso: string) => parseIsoDate(iso).getTime();

const MONTHLY = [
  { date: '2024-01-01', value: 100, revisionDate: '2024-02-15' },
  { date: '2024-02-01', value: 101.5, revisionDate: '2024-03-15' },
  { date: '2024-03-01', value: 103, revisionDate: '2024-04-15' },
];

function renderChart(props: Partial<SeriesChartProps> = {}) {
  return render(
    <ThemeProvider theme={createTheme()}>
      <SeriesChart data={MONTHLY} title='Test Series' units='Index' frequency='Monthly' {...props} />
    </ThemeProvider>
  );
}

beforeEach(() => {
  lineMock.mockClear();
});

describe('SeriesChart', () => {
  test('plots each point at its calendar date on a time axis', () => {
    renderChart();

    const { data, options } = lastLineProps();
    expect(options.scales.x.type).toBe('time');
    expect(data.datasets[0].data.map((p: any) => [p.x, p.y])).toEqual([
      [at('2024-01-01'), 100],
      [at('2024-02-01'), 101.5],
      [at('2024-03-01'), 103],
    ]);
    expect(screen.getByText(/Data Points: 3/)).toBeInTheDocument();
  });

  test('renders mixed daily and monthly points in date order', () => {
    renderChart({
      data: [
        { date: '2024-03-01', value: 3 },
        { date: '2024-01-15', value: 1.5 },
        { date: '2024-02-01', value: 2 },
        { date: '2024-01-01', value: 1 },
        { date: '2024-01-02', value: 1.1 },
      ],
    });

    const xs = lastLineProps().data.datasets[0].data.map((p: any) => p.x);
    expect(xs).toEqual(
      ['2024-01-01', '2024-01-02', '2024-01-15', '2024-02-01', '2024-03-01'].map(at)
    );
  });

  test('draws a point annotation as a line at its date and a range as a box over its dates', () => {
    const annotations: SeriesChartAnnotation[] = [
      { kind: 'point', id: 'a1', label: 'Rate cut', date: '2024-02-01', color: '#ff0000' },
      { kind: 'range', id: 'a2', label: 'Recession', startDate: '2024-01-01', endDate: '2024-02-15' },
    ];
    renderChart({ annotations });

    const drawn = lastLineProps().options.plugins.annotation.annotations;
    expect(drawn.a1).toMatchObject({
      type: 'line',
      xMin: at('2024-02-01'),
      xMax: at('2024-02-01'),
      borderColor: '#ff0000',
      label: { content: 'Rate cut' },
    });
    expect(drawn.a2).toMatchObject({
      type: 'box',
      xMin: at('2024-01-01'),
      // The end date is inclusive, so the box runs to the start of the next day.
      xMax: at('2024-02-16'),
      label: { content: 'Recession' },
    });
  });

  test('annotations are hit only where drawn and never widen the axis', () => {
    renderChart({
      annotations: [
        { kind: 'range', id: 'old', label: 'Old', startDate: '1929-08-01', endDate: '2024-01-31' },
      ],
    });

    const { annotation } = lastLineProps().options.plugins;
    expect(annotation.interaction).toEqual({ mode: 'nearest', intersect: true });
    expect(annotation.annotations.old.adjustScaleRange).toBe(false);
  });

  test('places a point annotation at its own date, not the nearest observation', () => {
    renderChart({ annotations: [{ kind: 'point', id: 'mid', label: 'Mid', date: '2024-02-15' }] });
    expect(lastLineProps().options.plugins.annotation.annotations.mid).toMatchObject({
      xMin: at('2024-02-15'),
      xMax: at('2024-02-15'),
    });
  });

  test('a range that leaves out every value says so instead of drawing an empty plot', () => {
    renderChart({
      dateRange: { start: parseIsoDate('2025-01-01'), end: null },
      emptyMessage: 'Too few points for this transformation',
    });
    expect(screen.queryByTestId('series-chart')).toBeNull();
    expect(screen.getByText(/No observations in the chosen date range/)).toBeInTheDocument();
    expect(screen.queryByText('Too few points for this transformation')).toBeNull();
  });

  // ECO-415: an inverted range left red fields, an empty plot with a backwards axis and no word.
  test('a start date after the end date says so instead of plotting', () => {
    renderChart({
      dateRange: { start: parseIsoDate('2024-03-01'), end: parseIsoDate('2024-01-01') },
      onDateRangeChange: vi.fn(),
    });
    expect(screen.queryByTestId('series-chart')).toBeNull();
    expect(
      screen.getByText(/The start date \(Mar 1, 2024\) is after the end date \(Jan 1, 2024\)/)
    ).toBeInTheDocument();
    expect(screen.queryByText(/No observations in the chosen date range/)).toBeNull();
  });

  test('a series with no values shows the empty message', () => {
    renderChart({
      data: [{ date: '2024-01-01', value: null, revisionDate: '2024-02-15' }],
      emptyMessage: 'Too few points for this transformation',
    });
    expect(screen.queryByTestId('series-chart')).toBeNull();
    expect(screen.getByText('Too few points for this transformation')).toBeInTheDocument();
  });

  test('a controlled range without a handler is read-only', () => {
    renderChart({ dateRange: { start: parseIsoDate('2024-02-01'), end: parseIsoDate('2024-03-01') } });
    const { options } = lastLineProps();
    expect(options.scales.x.min).toBe(at('2024-02-01'));
    expect(options.scales.x.max).toBe(at('2024-03-01'));
    const fromChip = screen.getByText(/^From:/).closest('.MuiChip-root') as HTMLElement;
    expect(within(fromChip).queryByTestId('CancelIcon')).toBeNull();
    const startSections = within(screen.getByRole('group', { name: 'Start Date' })).getAllByRole(
      'spinbutton'
    );
    for (const section of startSections) expect(section).toHaveAttribute('aria-readonly', 'true');
  });

  test('draws no annotations when none are given', () => {
    renderChart();
    expect(lastLineProps().options.plugins.annotation.annotations).toEqual({});
  });

  test('passes a clicked annotation id to onAnnotationClick', () => {
    const onAnnotationClick = vi.fn();
    renderChart({
      annotations: [{ kind: 'point', id: 'a1', label: 'Note', date: '2024-02-01' }],
      onAnnotationClick,
    });

    lastLineProps().options.plugins.annotation.annotations.a1.click();
    expect(onAnnotationClick).toHaveBeenCalledWith('a1');
  });

  test('a controlled date range limits points and annotations to it', async () => {
    const onDateRangeChange = vi.fn();
    renderChart({
      dateRange: { start: parseIsoDate('2024-02-01'), end: null },
      onDateRangeChange,
      annotations: [
        { kind: 'point', id: 'before', label: 'Before', date: '2024-01-01' },
        { kind: 'range', id: 'overlaps', label: 'Overlaps', startDate: '2023-12-01', endDate: '2024-02-01' },
      ],
    });

    const { data, options } = lastLineProps();
    expect(data.datasets[0].data.map((p: any) => p.x)).toEqual(
      ['2024-02-01', '2024-03-01'].map(at)
    );
    expect(Object.keys(options.plugins.annotation.annotations)).toEqual(['overlaps']);
    // Cut to the axis so its label, drawn at the box's left edge, stays visible.
    expect(options.plugins.annotation.annotations.overlaps.xMin).toBe(at('2024-02-01'));
    expect(options.scales.x.min).toBe(at('2024-02-01'));
    expect(options.scales.x.max).toBeUndefined();

    const fromChip = screen.getByText(/^From:/).closest('.MuiChip-root') as HTMLElement;
    await userEvent.click(within(fromChip).getByTestId('CancelIcon'));
    expect(onDateRangeChange).toHaveBeenCalledWith({ start: null, end: null });
  });

  test('without a dateRange prop the chart keeps its own range', async () => {
    renderChart();
    expect(lastLineProps().options.scales.x.min).toBeUndefined();

    const start = screen.getByRole('group', { name: 'Start Date' });
    await userEvent.click(within(start).getByRole('spinbutton', { name: 'Month' }));
    await userEvent.keyboard('02012024');

    expect(lastLineProps().data.datasets[0].data.map((p: any) => p.x)).toEqual(
      ['2024-02-01', '2024-03-01'].map(at)
    );
    expect(screen.getByText('From: Feb 1, 2024')).toBeInTheDocument();
  });

  test('tooltips show the calendar date, the value with units and a later revision date', () => {
    renderChart();

    const { title, label } = lastLineProps().options.plugins.tooltip.callbacks;
    const raw = lastLineProps().data.datasets[0].data[1];
    expect(title([{ raw }])).toBe('February 1, 2024');
    expect(
      label({ raw, parsed: { y: 101.5 }, dataset: { label: 'Test Series' } })
    ).toEqual(['Test Series: 101.5 Index', 'Revised: Mar 15, 2024']);
  });

  describe('quarterly and annual series (ECO-415)', () => {
    const QUARTERLY = [
      { date: '2024-01-01', value: 28_100_000_000_000 },
      { date: '2024-04-01', value: 28_300_000_000_000 },
      { date: '2024-07-01', value: 28_511_400_000_000 },
      { date: '2024-10-01', value: 28_624_702_000_000 },
    ];

    test('the tooltip names the quarter and shows whole-dollar levels without decimals', () => {
      renderChart({ data: QUARTERLY, frequency: 'Quarterly', units: 'Current Dollars' });
      const { title, label } = lastLineProps().options.plugins.tooltip.callbacks;
      const raw = lastLineProps().data.datasets[0].data[3];
      expect(title([{ raw }])).toBe('2024 Q4');
      expect(label({ raw, parsed: { y: 28_624_702_000_000 }, dataset: { label: 'GDP' } })).toEqual(
        ['GDP: 28,624,702,000,000 Current Dollars']
      );
    });

    test('a short quarterly axis ticks quarters, labelled as quarters', () => {
      renderChart({ data: QUARTERLY, frequency: 'Quarterly' });
      const { time } = lastLineProps().options.scales.x;
      expect(time.unit).toBe('quarter');
      expect(time.displayFormats.quarter).toBe("yyyy 'Q'q");
    });

    test('a long quarterly axis and an annual one tick years, never months', () => {
      renderChart({
        data: [
          { date: '1947-01-01', value: 243 },
          { date: '2024-10-01', value: 29_700 },
        ],
        frequency: 'Quarterly',
      });
      expect(lastLineProps().options.scales.x.time).toMatchObject({ minUnit: 'year' });
      expect(lastLineProps().options.scales.x.time.unit).toBeUndefined();

      renderChart({ frequency: 'Annual' });
      expect(lastLineProps().options.scales.x.time).toMatchObject({ minUnit: 'year' });
    });

    test('a monthly axis is left to Chart.js', () => {
      renderChart();
      const { time } = lastLineProps().options.scales.x;
      expect(time.unit).toBeUndefined();
      expect(time.minUnit).toBeUndefined();
    });

    test('trillions of dollars on the y-axis are scaled, not written out', () => {
      renderChart({ data: QUARTERLY, frequency: 'Quarterly' });
      const { callback } = lastLineProps().options.scales.y.ticks;
      const ticks = [28.0e12, 28.2e12, 28.4e12, 28.6e12].map(value => ({ value }));
      expect(ticks.map(t => callback(t.value, 0, ticks))).toEqual([
        '28.0T',
        '28.2T',
        '28.4T',
        '28.6T',
      ]);
    });

    test("small values keep Chart.js' own y-axis labels", () => {
      renderChart();
      expect(lastLineProps().options.scales.y.ticks.callback).toBeUndefined();
    });
  });

  test('shows the transformation in the title and offers the backend transformations', async () => {
    const onTransformationChange = vi.fn();
    renderChart({ transformation: 'YEAR_OVER_YEAR', onTransformationChange });

    expect(lastLineProps().options.plugins.title.text).toBe(
      'Test Series (Year-over-Year % Change)'
    );
    expect(lastLineProps().options.scales.y.title.text).toBe('Percent Change');

    await userEvent.click(screen.getByRole('combobox'));
    const options = screen.getAllByRole('option').map(option => option.textContent);
    expect(options).toEqual([
      'None',
      'Year-over-Year',
      'Quarter-over-Quarter',
      'Month-over-Month',
      'Change since first available value',
      'Log difference',
    ]);
    await userEvent.click(screen.getByRole('option', { name: 'Log difference' }));
    expect(onTransformationChange).toHaveBeenCalledWith('LOG_DIFFERENCE');
  });

  test('labels a percent change with % and levels with the series units', () => {
    renderChart({ transformation: 'YEAR_OVER_YEAR' });
    const { label } = lastLineProps().options.plugins.tooltip.callbacks;
    const raw = lastLineProps().data.datasets[0].data[1];
    expect(label({ raw, parsed: { y: 5.4 }, dataset: { label: 'Test Series' } })).toEqual([
      'Test Series: 5.40 %',
      'Revised: Mar 15, 2024',
    ]);

    renderChart({ transformation: 'NONE' });
    expect(lastLineProps().options.scales.y.title.text).toBe('Index');
    const levels = lastLineProps().data.datasets[0].data[1];
    expect(
      lastLineProps().options.plugins.tooltip.callbacks.label({
        raw: levels,
        parsed: { y: 101.5 },
        dataset: { label: 'Test Series' },
      })
    ).toEqual(['Test Series: 101.5 Index', 'Revised: Mar 15, 2024']);
  });

  test('labels log difference without a percent unit', () => {
    renderChart({ transformation: 'LOG_DIFFERENCE' });

    expect(lastLineProps().options.plugins.title.text).toBe('Test Series (Log Difference)');
    expect(lastLineProps().options.scales.y.title.text).toBe('Log Difference');

    const { label } = lastLineProps().options.plugins.tooltip.callbacks;
    const raw = lastLineProps().data.datasets[0].data[1];
    // Four significant digits: at two decimals a 1.2% rise would read 0.01 (ECO-407)
    expect(label({ raw, parsed: { y: 0.011879 }, dataset: { label: 'Test Series' } })).toEqual([
      'Test Series: 0.01188',
      'Revised: Mar 15, 2024',
    ]);
  });

  test('offers only transformations that fit a quarterly series', async () => {
    renderChart({ frequency: 'Quarterly', onTransformationChange: vi.fn() });

    await userEvent.setup().click(screen.getByRole('combobox', { name: 'Transformation' }));
    const options = screen.getAllByRole('option').map(o => o.textContent);
    expect(options).toContain('Quarter-over-Quarter');
    expect(options).toContain('Year-over-Year');
    expect(options).not.toContain('Month-over-Month');
  });

  describe('onPointClick', () => {
    test('is called with the shown observation nearest the click', () => {
      const onPointClick = vi.fn();
      renderChart({
        dateRange: { start: parseIsoDate('2024-02-01'), end: null },
        onPointClick,
      });

      // With the range applied, index 0 is February, not January.
      lastLineProps().options.onClick({ type: 'click' }, [{ datasetIndex: 0, index: 0 }]);
      expect(onPointClick).toHaveBeenCalledWith(MONTHLY[1]);
    });

    test('is not called for a click the annotation plugin already handled', () => {
      const onPointClick = vi.fn();
      const onAnnotationClick = vi.fn();
      renderChart({
        annotations: [{ kind: 'point', id: 'a1', label: 'Note', date: '2024-02-01' }],
        onAnnotationClick,
        onPointClick,
      });

      // Chart.js passes the same event to the plugin's click handler and then to onClick.
      const { options } = lastLineProps();
      const event = { type: 'click' };
      options.plugins.annotation.annotations.a1.click({}, event);
      options.onClick(event, [{ datasetIndex: 0, index: 1 }]);

      expect(onAnnotationClick).toHaveBeenCalledWith('a1');
      expect(onPointClick).not.toHaveBeenCalled();
    });

    test('leaves the chart without a click handler when not given', () => {
      renderChart();
      expect(lastLineProps().options.onClick).toBeUndefined();
    });
  });
});
