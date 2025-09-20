/**
 * Mock for d3-scale module
 * Provides mock implementations for scale functions
 */

// Mock scaleLinear function
export const scaleLinear = jest.fn(() => ({
  domain: jest.fn().mockReturnThis(),
  range: jest.fn().mockReturnThis(),
  rangeRound: jest.fn().mockReturnThis(),
  clamp: jest.fn().mockReturnThis(),
  interpolate: jest.fn().mockReturnThis(),
  unknown: jest.fn().mockReturnThis(),
  ticks: jest.fn().mockReturnValue([0, 0.25, 0.5, 0.75, 1]),
  tickFormat: jest.fn().mockReturnValue(jest.fn().mockReturnValue('0')),
  nice: jest.fn().mockReturnThis(),
  copy: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue(0.5),
}));

// Mock scaleTime function
export const scaleTime = jest.fn(() => ({
  domain: jest.fn().mockReturnThis(),
  range: jest.fn().mockReturnThis(),
  rangeRound: jest.fn().mockReturnThis(),
  clamp: jest.fn().mockReturnThis(),
  interpolate: jest.fn().mockReturnThis(),
  unknown: jest.fn().mockReturnThis(),
  ticks: jest.fn().mockReturnValue([]),
  tickFormat: jest.fn().mockReturnValue(jest.fn().mockReturnValue('')),
  nice: jest.fn().mockReturnThis(),
  copy: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue(new Date()),
}));

// Mock scaleOrdinal function
export const scaleOrdinal = jest.fn(() => ({
  domain: jest.fn().mockReturnThis(),
  range: jest.fn().mockReturnThis(),
  unknown: jest.fn().mockReturnThis(),
  copy: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue('blue'),
}));

// Mock scaleBand function
export const scaleBand = jest.fn(() => ({
  domain: jest.fn().mockReturnThis(),
  range: jest.fn().mockReturnThis(),
  rangeRound: jest.fn().mockReturnThis(),
  round: jest.fn().mockReturnThis(),
  paddingInner: jest.fn().mockReturnThis(),
  paddingOuter: jest.fn().mockReturnThis(),
  padding: jest.fn().mockReturnThis(),
  align: jest.fn().mockReturnThis(),
  bandwidth: jest.fn().mockReturnValue(10),
  step: jest.fn().mockReturnValue(15),
  copy: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue(0),
}));

// Mock scalePoint function
export const scalePoint = jest.fn(() => ({
  domain: jest.fn().mockReturnThis(),
  range: jest.fn().mockReturnThis(),
  rangeRound: jest.fn().mockReturnThis(),
  round: jest.fn().mockReturnThis(),
  padding: jest.fn().mockReturnThis(),
  align: jest.fn().mockReturnThis(),
  bandwidth: jest.fn().mockReturnValue(0),
  step: jest.fn().mockReturnValue(15),
  copy: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnValue(0),
}));

// Default export
export default {
  scaleLinear,
  scaleTime,
  scaleOrdinal,
  scaleBand,
  scalePoint,
};
