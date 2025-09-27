/**
 * Mock implementation of D3-Zoom for testing
 * PURPOSE: Provide predictable test behavior for D3 zoom components
 */

const { vi } = require('vitest');

module.exports = {
  zoom: vi.fn(() => ({
    scaleExtent: vi.fn(() => ({
      on: vi.fn(() => vi.fn()),
    })),
    on: vi.fn(() => vi.fn()),
    transform: vi.fn(() => vi.fn()),
  })),
  zoomTransform: vi.fn(() => ({
    x: 0,
    y: 0,
    k: 1,
    scale: vi.fn(() => 1),
    translate: vi.fn(() => [0, 0]),
  })),
  zoomIdentity: vi.fn(() => ({
    x: 0,
    y: 0,
    k: 1,
    scale: vi.fn(() => 1),
    translate: vi.fn(() => [0, 0]),
  })),
};
