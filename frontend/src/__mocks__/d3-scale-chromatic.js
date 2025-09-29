/**
 * Mock implementation of D3-Scale-Chromatic for testing
 * PURPOSE: Provide predictable test behavior for D3 chromatic scale components
 */

const { vi } = require('vitest');

module.exports = {
  schemeCategory10: ['#1f77b4', '#ff7f0e', '#2ca02c', '#d62728', '#9467bd'],
  schemeCategory20: ['#1f77b4', '#aec7e8', '#ff7f0e', '#ffbb78', '#2ca02c'],
  interpolateViridis: vi.fn(() => vi.fn()),
  interpolateInferno: vi.fn(() => vi.fn()),
  interpolateMagma: vi.fn(() => vi.fn()),
  interpolatePlasma: vi.fn(() => vi.fn()),
  interpolateWarm: vi.fn(() => vi.fn()),
  interpolateCool: vi.fn(() => vi.fn()),
  interpolateRainbow: vi.fn(() => vi.fn()),
  interpolateSinebow: vi.fn(() => vi.fn()),
  scaleSequential: vi.fn(() => ({
    domain: vi.fn(() => ({
      interpolator: vi.fn(() => vi.fn()),
    })),
  })),
};
