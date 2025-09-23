/**
 * Mock implementation of D3-Geo for testing
 * PURPOSE: Provide predictable test behavior for D3 geo components
 */

module.exports = {
  geoPath: jest.fn(() => jest.fn()),
  geoMercator: jest.fn(() => ({
    scale: jest.fn(() => ({
      translate: jest.fn(() => ({
        center: jest.fn(() => jest.fn()),
      })),
    })),
    translate: jest.fn(() => ({
      scale: jest.fn(() => jest.fn()),
    })),
  })),
  geoAlbersUsa: jest.fn(() => ({
    scale: jest.fn(() => ({
      translate: jest.fn(() => jest.fn()),
    })),
  })),
};
