/**
 * Mock implementation of D3-Scale-Chromatic for testing
 * PURPOSE: Provide predictable test behavior for D3 chromatic scale components
 */

module.exports = {
  schemeCategory10: ['#1f77b4', '#ff7f0e', '#2ca02c', '#d62728', '#9467bd'],
  schemeCategory20: ['#1f77b4', '#aec7e8', '#ff7f0e', '#ffbb78', '#2ca02c'],
  interpolateViridis: jest.fn(() => jest.fn()),
  interpolateInferno: jest.fn(() => jest.fn()),
  interpolateMagma: jest.fn(() => jest.fn()),
  interpolatePlasma: jest.fn(() => jest.fn()),
  interpolateWarm: jest.fn(() => jest.fn()),
  interpolateCool: jest.fn(() => jest.fn()),
  interpolateRainbow: jest.fn(() => jest.fn()),
  interpolateSinebow: jest.fn(() => jest.fn()),
  scaleSequential: jest.fn(() => ({
    domain: jest.fn(() => ({
      interpolator: jest.fn(() => jest.fn()),
    })),
  })),
};
