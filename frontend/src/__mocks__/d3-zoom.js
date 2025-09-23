/**
 * Mock implementation of D3-Zoom for testing
 * PURPOSE: Provide predictable test behavior for D3 zoom components
 */

module.exports = {
  zoom: jest.fn(() => ({
    scaleExtent: jest.fn(() => ({
      on: jest.fn(() => jest.fn()),
    })),
    on: jest.fn(() => jest.fn()),
    transform: jest.fn(() => jest.fn()),
  })),
  zoomTransform: jest.fn(() => ({
    x: 0,
    y: 0,
    k: 1,
    scale: jest.fn(() => 1),
    translate: jest.fn(() => [0, 0]),
  })),
  zoomIdentity: jest.fn(() => ({
    x: 0,
    y: 0,
    k: 1,
    scale: jest.fn(() => 1),
    translate: jest.fn(() => [0, 0]),
  })),
};
