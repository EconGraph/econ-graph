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

  test('a controlled range without a handler is read-only', () => {
    renderChart({ dateRange: { start: parseIsoDate('2024-02-01'), end: parseIsoDate('2024-03-01') } });
    const { options } = lastLineProps();
    expect(options.scales.x.min).toBe(at('2024-02-01'));
    expect(options.scales.x.max).toBe(at('2024-03-01'));
    const fromChip = screen.getByText(/^From:/).closest('.MuiChip-root') as HTMLElement;
    expect(within(fromChip).queryByTestId('CancelIcon')).toBeNull();
    expect(screen.getByTestId('date-picker-start')).toHaveAttribute('readonly');
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

    const start = screen.getByTestId('date-picker-start');
    await userEvent.click(start);
    await userEvent.keyboard('02/01/2024');

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
    ).toEqual(['Test Series: 101.50 Index', 'Revised: Mar 15, 2024']);
  });

  test('shows the transformation in the title and offers the backend transformations', async () => {
    const onTransformationChange = vi.fn();
    renderChart({ transformation: 'YEAR_OVER_YEAR', onTransformationChange });

    expect(lastLineProps().options.plugins.title.text).toBe(
      'Test Series (Year-over-Year % Change)'
    );
    expect(lastLineProps().options.scales.y.title.text).toBe('Percent Change');

    await userEvent.click(screen.getByRole('combobox'));
    await userEvent.click(screen.getByRole('option', { name: 'Month-over-Month' }));
    expect(onTransformationChange).toHaveBeenCalledWith('MONTH_OVER_MONTH');
  });
});
