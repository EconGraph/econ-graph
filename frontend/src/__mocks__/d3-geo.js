/**
 * Mock implementation of D3-Geo for testing
 * PURPOSE: Provide predictable test behavior for D3 geo components
 */

const { vi } = require('vitest');

module.exports = {
  geoPath: vi.fn(() => vi.fn()),
  geoMercator: vi.fn(() => ({
    scale: vi.fn(() => ({
      translate: vi.fn(() => ({
        center: vi.fn(() => vi.fn()),
      })),
    })),
    translate: vi.fn(() => ({
      scale: vi.fn(() => vi.fn()),
    })),
  })),
  geoAlbersUsa: vi.fn(() => ({
    scale: vi.fn(() => ({
      translate: vi.fn(() => vi.fn()),
    })),
  })),
};
