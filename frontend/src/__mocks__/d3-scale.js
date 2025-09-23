/**
 * Mock implementation of D3-Scale for testing
 * PURPOSE: Provide predictable test behavior for D3 scale components
 */

module.exports = {
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
  scaleOrdinal: jest.fn(() => ({
    domain: jest.fn(() => ({
      range: jest.fn(() => jest.fn()),
    })),
  })),
  scaleBand: jest.fn(() => ({
    domain: jest.fn(() => ({
      range: jest.fn(() => ({
        padding: jest.fn(() => jest.fn()),
      })),
    })),
  })),
  scalePoint: jest.fn(() => ({
    domain: jest.fn(() => ({
      range: jest.fn(() => ({
        padding: jest.fn(() => jest.fn()),
      })),
    })),
  })),
};
