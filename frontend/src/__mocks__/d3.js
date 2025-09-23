/**
 * Mock implementation of D3.js for testing
 * PURPOSE: Provide predictable test behavior for D3 components
 */

module.exports = {
  select: jest.fn(() => ({
    selectAll: jest.fn(() => ({
      data: jest.fn(() => ({
        enter: jest.fn(() => ({
          append: jest.fn(() => ({
            attr: jest.fn(() => ({
              style: jest.fn(() => ({})),
            })),
            style: jest.fn(() => ({})),
          })),
        })),
      })),
    })),
    attr: jest.fn(() => ({})),
    style: jest.fn(() => ({})),
    append: jest.fn(() => ({
      attr: jest.fn(() => ({})),
      style: jest.fn(() => ({})),
    })),
  })),
  scaleLinear: jest.fn(() => ({
    domain: jest.fn(() => ({
      range: jest.fn(() => ({
        interpolate: jest.fn(() => jest.fn()),
      })),
    })),
    interpolate: jest.fn(() => jest.fn()),
  })),
  scaleTime: jest.fn(() => ({
    domain: jest.fn(() => ({
      range: jest.fn(() => ({
        interpolate: jest.fn(() => jest.fn()),
      })),
    })),
    interpolate: jest.fn(() => jest.fn()),
  })),
  line: jest.fn(() => ({
    x: jest.fn(() => ({
      y: jest.fn(() => jest.fn()),
    })),
    y: jest.fn(() => jest.fn()),
  })),
  area: jest.fn(() => ({
    x: jest.fn(() => ({
      y0: jest.fn(() => ({
        y1: jest.fn(() => jest.fn()),
      })),
    })),
    y1: jest.fn(() => jest.fn()),
  })),
  axisBottom: jest.fn(() => jest.fn()),
  axisLeft: jest.fn(() => jest.fn()),
  extent: jest.fn(() => [0, 100]),
  max: jest.fn(() => 100),
  min: jest.fn(() => 0),
  format: jest.fn(() => jest.fn(d => d.toString())),
  timeParse: jest.fn(() => jest.fn()),
  timeFormat: jest.fn(() => jest.fn()),
};
