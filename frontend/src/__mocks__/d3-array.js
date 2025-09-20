/**
 * Mock for d3-array module
 * Provides mock implementations for array utility functions
 */

// Mock extent function
export const extent = jest.fn().mockReturnValue([0, 100]);

// Mock min function
export const min = jest.fn().mockReturnValue(0);

// Mock max function
export const max = jest.fn().mockReturnValue(100);

// Mock sum function
export const sum = jest.fn().mockReturnValue(500);

// Mock mean function
export const mean = jest.fn().mockReturnValue(50);

// Mock median function
export const median = jest.fn().mockReturnValue(50);

// Mock quantile function
export const quantile = jest.fn().mockReturnValue(25);

// Mock bisect function
export const bisect = jest.fn().mockReturnValue(5);

// Mock bisectLeft function
export const bisectLeft = jest.fn().mockReturnValue(5);

// Mock bisectRight function
export const bisectRight = jest.fn().mockReturnValue(5);

// Mock bisector function
export const bisector = jest.fn().mockReturnValue({
  left: jest.fn().mockReturnValue(5),
  right: jest.fn().mockReturnValue(5),
});

// Mock ascending function
export const ascending = jest.fn().mockReturnValue(-1);

// Mock descending function
export const descending = jest.fn().mockReturnValue(1);

// Mock shuffle function
export const shuffle = jest.fn().mockReturnValue([1, 2, 3, 4, 5]);

// Mock ticks function
export const ticks = jest.fn().mockReturnValue([0, 25, 50, 75, 100]);

// Mock tickStep function
export const tickStep = jest.fn().mockReturnValue(25);

// Mock tickIncrement function
export const tickIncrement = jest.fn().mockReturnValue(1);

// Mock range function
export const range = jest.fn().mockReturnValue([0, 1, 2, 3, 4]);

// Mock transpose function
export const transpose = jest.fn().mockReturnValue([
  [1, 2],
  [3, 4],
]);

// Mock zip function
export const zip = jest.fn().mockReturnValue([
  [1, 3],
  [2, 4],
]);

// Default export
export default {
  extent,
  min,
  max,
  sum,
  mean,
  median,
  quantile,
  bisect,
  bisectLeft,
  bisectRight,
  bisector,
  ascending,
  descending,
  shuffle,
  ticks,
  tickStep,
  tickIncrement,
  range,
  transpose,
  zip,
};
