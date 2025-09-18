/**
 * Mock for d3-zoom module
 * Provides mock implementations for zoom behavior functions
 */

// Mock zoom function
export const zoom = jest.fn(() => ({
  scaleExtent: jest.fn().mockReturnThis(),
  translateExtent: jest.fn().mockReturnThis(),
  wheelDelta: jest.fn().mockReturnThis(),
  clickDistance: jest.fn().mockReturnThis(),
  duration: jest.fn().mockReturnThis(),
  interpolate: jest.fn().mockReturnThis(),
  filter: jest.fn().mockReturnThis(),
  touchable: jest.fn().mockReturnThis(),
  on: jest.fn().mockReturnThis(),
  transform: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnThis(),
}));

// Mock zoomTransform function
export const zoomTransform = jest.fn().mockReturnValue({
  x: 0,
  y: 0,
  k: 1,
  scale: jest.fn().mockReturnValue(1),
  translate: jest.fn().mockReturnValue([0, 0]),
  apply: jest.fn().mockReturnValue([0, 0]),
  applyX: jest.fn().mockReturnValue(0),
  applyY: jest.fn().mockReturnValue(0),
  invert: jest.fn().mockReturnValue([0, 0]),
  invertX: jest.fn().mockReturnValue(0),
  invertY: jest.fn().mockReturnValue(0),
  rescaleX: jest.fn().mockReturnValue({
    domain: jest.fn().mockReturnThis(),
    range: jest.fn().mockReturnThis(),
    copy: jest.fn().mockReturnThis(),
  }),
  rescaleY: jest.fn().mockReturnValue({
    domain: jest.fn().mockReturnThis(),
    range: jest.fn().mockReturnThis(),
    copy: jest.fn().mockReturnThis(),
  }),
  toString: jest.fn().mockReturnValue('translate(0,0) scale(1)'),
});

// Mock zoomIdentity function
export const zoomIdentity = {
  x: 0,
  y: 0,
  k: 1,
  scale: jest.fn().mockReturnValue(1),
  translate: jest.fn().mockReturnValue([0, 0]),
  apply: jest.fn().mockReturnValue([0, 0]),
  applyX: jest.fn().mockReturnValue(0),
  applyY: jest.fn().mockReturnValue(0),
  invert: jest.fn().mockReturnValue([0, 0]),
  invertX: jest.fn().mockReturnValue(0),
  invertY: jest.fn().mockReturnValue(0),
  rescaleX: jest.fn().mockReturnValue({
    domain: jest.fn().mockReturnThis(),
    range: jest.fn().mockReturnThis(),
    copy: jest.fn().mockReturnThis(),
  }),
  rescaleY: jest.fn().mockReturnValue({
    domain: jest.fn().mockReturnThis(),
    range: jest.fn().mockReturnThis(),
    copy: jest.fn().mockReturnThis(),
  }),
  toString: jest.fn().mockReturnValue('translate(0,0) scale(1)'),
};

// Mock zoomBehavior function
export const zoomBehavior = jest.fn(() => ({
  scaleExtent: jest.fn().mockReturnThis(),
  translateExtent: jest.fn().mockReturnThis(),
  wheelDelta: jest.fn().mockReturnThis(),
  clickDistance: jest.fn().mockReturnThis(),
  duration: jest.fn().mockReturnThis(),
  interpolate: jest.fn().mockReturnThis(),
  filter: jest.fn().mockReturnThis(),
  touchable: jest.fn().mockReturnThis(),
  on: jest.fn().mockReturnThis(),
  transform: jest.fn().mockReturnThis(),
  __call__: jest.fn().mockReturnThis(),
}));

// Default export
export default {
  zoom,
  zoomTransform,
  zoomIdentity,
  zoomBehavior,
};
