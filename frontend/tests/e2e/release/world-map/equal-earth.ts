// Copyright (c) 2026 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { expect, type Page } from '@playwright/test';
import { geoArea, geoEqualEarth, geoPath } from 'd3-geo';
import { feature } from 'topojson-client';
import type { WorldAtlasTopology } from '../../../../src/components/global/worldAtlas';

const require = createRequire(import.meta.url);
const atlas = JSON.parse(
  readFileSync(require.resolve('world-atlas/countries-50m.json'), 'utf8')
) as WorldAtlasTopology;
const countries = feature(atlas, atlas.objects.countries).features;
const countryFeature = (id: number) => countries.find(country => Number(country.id) === id)!;

/** Check the drawing, not just the projection selector's label. */
export async function expectEqualEarthGeometry(page: Page) {
  await expect(page.getByRole('combobox', { name: 'Projection' })).toHaveText('Equal Earth');
  const projection = geoEqualEarth().fitSize([960, 500], { type: 'Sphere' });
  const path = geoPath(projection);
  for (const id of [276, 180, 304]) {
    await expect(page.locator(`path.country[data-iso-numeric="${id}"]`)).toHaveAttribute(
      'd',
      path(countryFeature(id))!
    );
  }
}

/**
 * Rasterize the actual DOM path without borders or overlays. Congo and Greenland have similar
 * spherical areas but very different latitudes, so their ratio detects a compromise projection.
 * Supersampling and alpha weighting keep the error below 2% even on the small Greenland mask.
 */
export async function expectEqualLandAreas(page: Page) {
  const ids = [180, 304];
  const renderedAreas = await page.evaluate(countryIds => {
    const canvas = document.createElement('canvas');
    const sampleScale = 4;
    canvas.width = 960 * sampleScale;
    canvas.height = 500 * sampleScale;
    const context = canvas.getContext('2d', { willReadFrequently: true })!;
    return countryIds.map(id => {
      context.resetTransform();
      context.clearRect(0, 0, canvas.width, canvas.height);
      context.scale(sampleScale, sampleScale);
      const path = document.querySelector(`path.country[data-iso-numeric="${id}"]`)!;
      context.fill(new Path2D(path.getAttribute('d')!));
      const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
      let area = 0;
      for (let i = 3; i < pixels.length; i += 4) area += pixels[i] / 255;
      return area;
    });
  }, ids);
  const expectedRatio = geoArea(countryFeature(ids[0])) / geoArea(countryFeature(ids[1]));
  expect(renderedAreas[0]).toBeGreaterThan(0);
  expect(renderedAreas[1]).toBeGreaterThan(0);
  expect(Math.abs(renderedAreas[0] / renderedAreas[1] / expectedRatio - 1)).toBeLessThan(0.02);
}
