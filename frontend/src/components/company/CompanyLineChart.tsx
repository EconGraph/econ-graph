import React from 'react';
import { Alert, Box, Typography } from '@mui/material';
import type { CompanyValue } from '../../utils/companyResearch';

/** Plot one concept/unit with its true duration visible; exact values stay in the table/CSV. */
export default function CompanyLineChart({
  values,
  label,
}: {
  values: readonly CompanyValue[];
  label: string;
}) {
  const points = values
    .map(value => ({ ...value, approximate: Number(value.value) }))
    .filter(value => Number.isFinite(value.approximate))
    .sort((a, b) => a.periodEnd.localeCompare(b.periodEnd));
  if (!points.length)
    return <Alert severity='info'>No numeric values are available for this line.</Alert>;
  const min = Math.min(...points.map(point => point.approximate));
  const max = Math.max(...points.map(point => point.approximate));
  const range = max - min || 1;
  const firstDate = Date.parse(points[0].periodEnd);
  const dateRange = Date.parse(points[points.length - 1].periodEnd) - firstDate || 1;
  const x = (point: (typeof points)[number]) =>
    20 + ((Date.parse(point.periodEnd) - firstDate) * 560) / dateRange;
  const path = points
    .map(point => `${x(point)},${180 - ((point.approximate - min) * 150) / range}`)
    .join(' ');
  return (
    <Box component='figure' sx={{ m: 0 }}>
      <Typography component='figcaption'>
        {label} ({points[0].unit})
      </Typography>
      <svg
        viewBox='0 0 600 220'
        role='img'
        aria-label={`${label} line chart`}
        style={{ width: '100%', maxWidth: 800 }}
      >
        <title>{label}: approximate plotted values, exact values in table and CSV</title>
        <line x1='20' y1='180' x2='580' y2='180' stroke='currentColor' />
        <polyline points={path} fill='none' stroke='currentColor' strokeWidth='2' />
        {points.map(point => (
          <circle
            key={`${point.periodStart}-${point.periodEnd}-${point.accession}`}
            cx={x(point)}
            cy={180 - ((point.approximate - min) * 150) / range}
            r='4'
            fill='currentColor'
          >
            <title>{`${point.periodStart ?? 'Instant'} to ${point.periodEnd}: ${point.value} ${point.unit}`}</title>
          </circle>
        ))}
        <text x='20' y='210' fill='currentColor' fontSize='12'>
          {points[0].periodEnd}
        </text>
        <text x='580' y='210' textAnchor='end' fill='currentColor' fontSize='12'>
          {points[points.length - 1].periodEnd}
        </text>
      </svg>
      <Typography variant='caption'>
        Plot positions are approximate; the table and CSV preserve exact reported decimals.
      </Typography>
    </Box>
  );
}
