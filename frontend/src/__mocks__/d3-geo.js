/**
 * Mock for d3-geo module
 * Provides mock implementations for geographic projection functions
 */

// Mock geoPath function
export const geoPath = jest.fn(() => ({
  projection: jest.fn().mockReturnThis(),
  pointRadius: jest.fn().mockReturnThis(),
  context: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue('M0,0L10,10Z'), // Mock SVG path
}));

// Mock geoNaturalEarth1 projection
export const geoNaturalEarth1 = jest.fn(() => ({
  scale: jest.fn().mockReturnThis(),
  translate: jest.fn().mockReturnThis(),
  center: jest.fn().mockReturnThis(),
  rotate: jest.fn().mockReturnThis(),
  precision: jest.fn().mockReturnThis(),
  clipExtent: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue([0, 0]), // Mock projected coordinates
}));

// Mock geoMercator projection
export const geoMercator = jest.fn(() => ({
  scale: jest.fn().mockReturnThis(),
  translate: jest.fn().mockReturnThis(),
  center: jest.fn().mockReturnThis(),
  rotate: jest.fn().mockReturnThis(),
  precision: jest.fn().mockReturnThis(),
  clipExtent: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue([0, 0]), // Mock projected coordinates
}));

// Mock geoOrthographic projection
export const geoOrthographic = jest.fn(() => ({
  scale: jest.fn().mockReturnThis(),
  translate: jest.fn().mockReturnThis(),
  center: jest.fn().mockReturnThis(),
  rotate: jest.fn().mockReturnThis(),
  precision: jest.fn().mockReturnThis(),
  clipExtent: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue([0, 0]), // Mock projected coordinates
}));

// Mock geoAlbers projection
export const geoAlbers = jest.fn(() => ({
  scale: jest.fn().mockReturnThis(),
  translate: jest.fn().mockReturnThis(),
  center: jest.fn().mockReturnThis(),
  rotate: jest.fn().mockReturnThis(),
  precision: jest.fn().mockReturnThis(),
  clipExtent: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue([0, 0]), // Mock projected coordinates
}));

// Mock geoEqualEarth projection
export const geoEqualEarth = jest.fn(() => ({
  scale: jest.fn().mockReturnThis(),
  translate: jest.fn().mockReturnThis(),
  center: jest.fn().mockReturnThis(),
  rotate: jest.fn().mockReturnThis(),
  precision: jest.fn().mockReturnThis(),
  clipExtent: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue([0, 0]), // Mock projected coordinates
}));

// Mock geoGraticule function
export const geoGraticule = jest.fn(() => ({
  lines: jest.fn().mockReturnValue([]),
  outline: jest.fn().mockReturnValue({ type: 'Polygon', coordinates: [] }),
  step: jest.fn().mockReturnThis(),
  stepMajor: jest.fn().mockReturnThis(),
  stepMinor: jest.fn().mockReturnThis(),
  precision: jest.fn().mockReturnThis(),
  extent: jest.fn().mockReturnThis(),
  extentMajor: jest.fn().mockReturnThis(),
  extentMinor: jest.fn().mockReturnThis(),
}));

// Mock geoCircle function
export const geoCircle = jest.fn(() => ({
  center: jest.fn().mockReturnThis(),
  radius: jest.fn().mockReturnThis(),
  precision: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue({ type: 'Polygon', coordinates: [] }),
}));

// Mock geoDistance function
export const geoDistance = jest.fn().mockReturnValue(1000);

// Mock geoCentroid function
export const geoCentroid = jest.fn().mockReturnValue([0, 0]);

// Mock geoBounds function
export const geoBounds = jest.fn().mockReturnValue([
  [0, 0],
  [10, 10],
]);

// Mock geoArea function
export const geoArea = jest.fn().mockReturnValue(100);

// Mock geoLength function
export const geoLength = jest.fn().mockReturnValue(50);

// Default export
export default {
  geoPath,
  geoNaturalEarth1,
  geoMercator,
  geoOrthographic,
  geoAlbers,
  geoEqualEarth,
  geoGraticule,
  geoCircle,
  geoDistance,
  geoCentroid,
  geoBounds,
  geoArea,
  geoLength,
};
