/**
 * Mock implementation of D3-Array for testing
 * PURPOSE: Provide predictable test behavior for D3 array components
 */

module.exports = {
  extent: jest.fn(() => [0, 100]),
  max: jest.fn(() => 100),
  min: jest.fn(() => 0),
  mean: jest.fn(() => 50),
  median: jest.fn(() => 50),
  sum: jest.fn(() => 500),
  range: jest.fn(() => [0, 1, 2, 3, 4]),
  bisectLeft: jest.fn(() => 0),
  bisectRight: jest.fn(() => 0),
  bisectCenter: jest.fn(() => 0),
  bisector: jest.fn(() => jest.fn()),
  shuffle: jest.fn(array => array.slice()),
  ticks: jest.fn(() => [0, 25, 50, 75, 100]),
  tickStep: jest.fn(() => 25),
  tickIncrement: jest.fn(() => 25),
};
