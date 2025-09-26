/**
 * Mock implementation of D3-Array for testing
 * PURPOSE: Provide predictable test behavior for D3 array components
 */

const { vi } = require('vitest');

module.exports = {
  extent: vi.fn(() => [0, 100]),
  max: vi.fn(() => 100),
  min: vi.fn(() => 0),
  mean: vi.fn(() => 50),
  median: vi.fn(() => 50),
  sum: vi.fn(() => 500),
  range: vi.fn(() => [0, 1, 2, 3, 4]),
  bisectLeft: vi.fn(() => 0),
  bisectRight: vi.fn(() => 0),
  bisectCenter: vi.fn(() => 0),
  bisector: vi.fn(() => vi.fn()),
  shuffle: vi.fn(array => array.slice()),
  ticks: vi.fn(() => [0, 25, 50, 75, 100]),
  tickStep: vi.fn(() => 25),
  tickIncrement: vi.fn(() => 25),
};
