/**
 * Mock implementation of D3.js for testing
 * PURPOSE: Provide predictable test behavior for D3 components
 */

const { vi } = require('vitest');

module.exports = {
  select: vi.fn(() => ({
    selectAll: vi.fn(() => ({
      data: vi.fn(() => ({
        enter: vi.fn(() => ({
          append: vi.fn(() => ({
            attr: vi.fn(() => ({
              style: vi.fn(() => ({})),
            })),
            style: vi.fn(() => ({})),
          })),
        })),
      })),
    })),
    attr: vi.fn(() => ({})),
    style: vi.fn(() => ({})),
    append: vi.fn(() => ({
      attr: vi.fn(() => ({})),
      style: vi.fn(() => ({})),
    })),
  })),
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
  line: vi.fn(() => ({
    x: vi.fn(() => ({
      y: vi.fn(() => vi.fn()),
    })),
    y: vi.fn(() => vi.fn()),
  })),
  area: vi.fn(() => ({
    x: vi.fn(() => ({
      y0: vi.fn(() => ({
        y1: vi.fn(() => vi.fn()),
      })),
    })),
    y1: vi.fn(() => vi.fn()),
  })),
  axisBottom: vi.fn(() => vi.fn()),
  axisLeft: vi.fn(() => vi.fn()),
  extent: vi.fn(() => [0, 100]),
  max: vi.fn(() => 100),
  min: vi.fn(() => 0),
  format: vi.fn(() => vi.fn(d => d.toString())),
  timeParse: vi.fn(() => vi.fn()),
  timeFormat: vi.fn(() => vi.fn()),
};
