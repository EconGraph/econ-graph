/**
 * Mock implementation of D3-Scale for testing
 * PURPOSE: Provide predictable test behavior for D3 scale components
 */

const { vi } = require('vitest');

module.exports = {
  scaleLinear: vi.fn(() => ({
    domain: vi.fn(() => ({
      range: vi.fn(() => ({
        interpolate: vi.fn(() => vi.fn()),
      })),
    })),
    interpolate: vi.fn(() => vi.fn()),
  })),
  scaleTime: vi.fn(() => ({
    domain: vi.fn(() => ({
      range: vi.fn(() => ({
        interpolate: vi.fn(() => vi.fn()),
      })),
    })),
    interpolate: vi.fn(() => vi.fn()),
  })),
  scaleOrdinal: vi.fn(() => ({
    domain: vi.fn(() => ({
      range: vi.fn(() => vi.fn()),
    })),
  })),
  scaleBand: vi.fn(() => ({
    domain: vi.fn(() => ({
      range: vi.fn(() => ({
        padding: vi.fn(() => vi.fn()),
      })),
    })),
  })),
  scalePoint: vi.fn(() => ({
    domain: vi.fn(() => ({
      range: vi.fn(() => ({
        padding: vi.fn(() => vi.fn()),
      })),
    })),
  })),
};
