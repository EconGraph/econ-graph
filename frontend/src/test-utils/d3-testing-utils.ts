/**
 * D3.js Testing Utilities.
 *
 * Custom utilities for testing D3.js components with proper mocking
 * and DOM simulation capabilities.
 */

import { vi } from 'vitest';

/**
 * Creates a mock D3.js projection with all necessary methods
 */
export const createMockProjection = () => ({
  scale: vi.fn().mockReturnThis(),
  center: vi.fn().mockReturnThis(),
  translate: vi.fn().mockReturnThis(),
  clipAngle: vi.fn().mockReturnThis(),
  precision: vi.fn().mockReturnThis(),
  rotate: vi.fn().mockReturnThis(),
  fitSize: vi.fn().mockReturnThis(),
  fitExtent: vi.fn().mockReturnThis(),
  stream: vi.fn(),
  point: vi.fn(),
  invert: vi.fn(),
});

/**
 * Creates a mock D3.js path generator
 */
export const createMockPath = () => ({
  projection: vi.fn().mockReturnThis(),
  centroid: vi.fn(() => [100, 100]),
  area: vi.fn(() => 1000),
  bounds: vi.fn(() => ({ x: 0, y: 0, width: 100, height: 100 })),
  measure: vi.fn(() => 100),
});

/**
 * Creates a mock D3.js zoom behavior
 */
export const createMockZoom = () => ({
  scaleExtent: vi.fn().mockReturnThis(),
  translateExtent: vi.fn().mockReturnThis(),
  on: vi.fn().mockReturnThis(),
  filter: vi.fn().mockReturnThis(),
  wheelDelta: vi.fn().mockReturnThis(),
  clickDistance: vi.fn().mockReturnThis(),
  touchable: vi.fn().mockReturnThis(),
  tapDistance: vi.fn().mockReturnThis(),
  duration: vi.fn().mockReturnThis(),
  interpolate: vi.fn().mockReturnThis(),
});

/**
 * Creates a mock D3.js selection
 */
export const createMockSelection = (): any => ({
  select: vi.fn(() => createMockSelection()),
  selectAll: vi.fn(() => createMockSelection()),
  append: vi.fn(() => createMockSelection()),
  attr: vi.fn().mockReturnThis(),
  style: vi.fn().mockReturnThis(),
  classed: vi.fn().mockReturnThis(),
  text: vi.fn().mockReturnThis(),
  html: vi.fn().mockReturnThis(),
  property: vi.fn().mockReturnThis(),
  datum: vi.fn().mockReturnThis(),
  data: vi.fn(() => ({
    enter: vi.fn(() => ({
      append: vi.fn(() => createMockSelection()),
      merge: vi.fn(() => createMockSelection()),
    })),
    exit: vi.fn(() => ({
      remove: vi.fn(),
    })),
    merge: vi.fn(() => createMockSelection()),
  })),
  on: vi.fn().mockReturnThis(),
  call: vi.fn().mockReturnThis(),
  node: vi.fn(() => ({
    getBoundingClientRect: vi.fn(() => ({ width: 800, height: 600 })),
    getBBox: vi.fn(() => ({ x: 0, y: 0, width: 100, height: 100 })),
  })),
  transition: vi.fn(() => ({
    duration: vi.fn().mockReturnThis(),
    delay: vi.fn().mockReturnThis(),
    ease: vi.fn().mockReturnThis(),
    call: vi.fn().mockReturnThis(),
    on: vi.fn().mockReturnThis(),
  })),
  remove: vi.fn(),
});

/**
 * Creates a mock D3.js transform
 */
export const createMockTransform = () => ({
  k: 1,
  x: 0,
  y: 0,
  scale: vi.fn(),
  translate: vi.fn(),
  apply: vi.fn(),
  applyX: vi.fn(),
  applyY: vi.fn(),
  invert: vi.fn(),
  invertX: vi.fn(),
  invertY: vi.fn(),
  rescaleX: vi.fn(),
  rescaleY: vi.fn(),
  toString: vi.fn(() => 'translate(0,0) scale(1)'),
});

/**
 * Creates a mock D3.js identity transform
 */
export const createMockIdentity = () => ({
  translate: vi.fn(() => createMockTransform()),
  scale: vi.fn(() => createMockTransform()),
  k: 1,
  x: 0,
  y: 0,
});

/**
 * Sets up comprehensive D3.js mocks for testing
 */
export const setupD3Mocks = () => {
  const mockProjection = createMockProjection();
  const mockPath = createMockPath();
  const mockZoom = createMockZoom();
  const mockSelection = createMockSelection();
  const mockTransform = createMockTransform();
  const mockIdentity = createMockIdentity();

  // Mock d3-geo
  vi.mock('d3-geo', () => ({
    geoPath: vi.fn(() => mockPath),
    geoNaturalEarth1: vi.fn(() => mockProjection),
    geoMercator: vi.fn(() => mockProjection),
    geoOrthographic: vi.fn(() => mockProjection),
    geoEqualEarth: vi.fn(() => mockProjection),
    geoAlbers: vi.fn(() => mockProjection),
    geoAlbersUsa: vi.fn(() => mockProjection),
    geoAzimuthalEqualArea: vi.fn(() => mockProjection),
    geoAzimuthalEquidistant: vi.fn(() => mockProjection),
    geoConicConformal: vi.fn(() => mockProjection),
    geoConicEqualArea: vi.fn(() => mockProjection),
    geoConicEquidistant: vi.fn(() => mockProjection),
    geoEquirectangular: vi.fn(() => mockProjection),
    geoGnomonic: vi.fn(() => mockProjection),
    geoMollweide: vi.fn(() => mockProjection),
    geoRobinson: vi.fn(() => mockProjection),
    geoStereographic: vi.fn(() => mockProjection),
    geoTransverseMercator: vi.fn(() => mockProjection),
  }));

  // Mock d3-zoom
  vi.mock('d3-zoom', () => ({
    zoom: vi.fn(() => mockZoom),
    zoomTransform: vi.fn(() => mockTransform),
    zoomIdentity: mockIdentity,
  }));

  // Mock d3
  vi.mock('d3', () => ({
    select: vi.fn(() => mockSelection),
    selectAll: vi.fn(() => mockSelection),
    event: vi.fn(),
    mouse: vi.fn(() => [0, 0]),
    touch: vi.fn(() => [{ x: 0, y: 0 }]),
    touches: vi.fn(() => [{ x: 0, y: 0 }]),
    clientPoint: vi.fn(() => [0, 0]),
    zoomTransform: vi.fn(() => mockTransform),
    zoomIdentity: mockIdentity,
  }));

  // Mock d3-scale
  vi.mock('d3-scale', () => ({
    scaleSequential: vi.fn(() => ({
      domain: vi.fn().mockReturnThis(),
      range: vi.fn().mockReturnThis(),
      interpolator: vi.fn().mockReturnThis(),
      clamp: vi.fn().mockReturnThis(),
      unknown: vi.fn().mockReturnThis(),
    })),
    scaleLinear: vi.fn(() => ({
      domain: vi.fn().mockReturnThis(),
      range: vi.fn().mockReturnThis(),
      clamp: vi.fn().mockReturnThis(),
      unknown: vi.fn().mockReturnThis(),
    })),
    scaleOrdinal: vi.fn(() => ({
      domain: vi.fn().mockReturnThis(),
      range: vi.fn().mockReturnThis(),
      unknown: vi.fn().mockReturnThis(),
    })),
  }));

  // Mock d3-scale-chromatic
  vi.mock('d3-scale-chromatic', () => ({
    interpolateViridis: vi.fn(),
    interpolateBlues: vi.fn(),
    interpolateReds: vi.fn(),
    interpolateGreens: vi.fn(),
    interpolateOranges: vi.fn(),
    interpolatePurples: vi.fn(),
    interpolateBuGn: vi.fn(),
    interpolateBuPu: vi.fn(),
    interpolateGnBu: vi.fn(),
    interpolateOrRd: vi.fn(),
    interpolatePuBu: vi.fn(),
    interpolatePuBuGn: vi.fn(),
    interpolatePuRd: vi.fn(),
    interpolateRdPu: vi.fn(),
    interpolateYlGn: vi.fn(),
    interpolateYlGnBu: vi.fn(),
    interpolateYlOrBr: vi.fn(),
    interpolateYlOrRd: vi.fn(),
  }));

  // Mock d3-array
  vi.mock('d3-array', () => ({
    extent: vi.fn(() => [0, 100]),
    min: vi.fn(() => 0),
    max: vi.fn(() => 100),
    mean: vi.fn(() => 50),
    median: vi.fn(() => 50),
    quantile: vi.fn(() => 50),
    bisect: vi.fn(() => 0),
    bisectLeft: vi.fn(() => 0),
    bisectRight: vi.fn(() => 0),
    bisector: vi.fn(() => ({
      left: vi.fn(() => 0),
      right: vi.fn(() => 0),
    })),
    ascending: vi.fn(),
    descending: vi.fn(),
    scan: vi.fn(() => 0),
    shuffle: vi.fn(() => []),
    ticks: vi.fn(() => [0, 25, 50, 75, 100]),
    tickIncrement: vi.fn(() => 25),
    tickStep: vi.fn(() => 25),
    nice: vi.fn(() => [0, 100]),
    thresholdSturges: vi.fn(() => 5),
    histogram: vi.fn(() => ({
      value: vi.fn().mockReturnThis(),
      domain: vi.fn().mockReturnThis(),
      thresholds: vi.fn().mockReturnThis(),
    })),
  }));

  // Mock d3-selection
  vi.mock('d3-selection', () => ({
    select: vi.fn(() => mockSelection),
    selectAll: vi.fn(() => mockSelection),
    event: vi.fn(),
    mouse: vi.fn(() => [0, 0]),
    touch: vi.fn(() => [{ x: 0, y: 0 }]),
    touches: vi.fn(() => [{ x: 0, y: 0 }]),
    clientPoint: vi.fn(() => [0, 0]),
  }));

  // Mock the entire useWorldMap module to avoid module loading issues
  vi.mock('../../components/global/hooks/useWorldMap', () => ({
    useWorldMap: vi.fn(() => ({
      projection: mockProjection,
      path: mockPath,
      zoomBehavior: mockZoom,
      zoomToFit: vi.fn(),
      zoomToCountry: vi.fn(),
      resetZoom: vi.fn(),
      getZoomLevel: vi.fn(() => 1),
      getCenter: vi.fn(() => [0, 0]),
      projections: ['naturalEarth', 'mercator', 'orthographic'],
    })),
  }));

  return {
    mockProjection,
    mockPath,
    mockZoom,
    mockSelection,
    mockTransform,
    mockIdentity,
  };
};

/**
 * Creates a mock SVG element for testing
 */
export const createMockSVG = () => {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('width', '800');
  svg.setAttribute('height', '600');
  return svg;
};

/**
 * Creates a mock world atlas data for testing
 */
export const createMockWorldAtlasData = () => ({
  type: 'Topology',
  objects: {
    countries: {
      type: 'GeometryCollection',
      geometries: [
        {
          type: 'Polygon',
          properties: { ISO_A2: 'US', NAME: 'United States' },
          arcs: [
            [
              [0, 0],
              [1, 0],
              [1, 1],
              [0, 1],
              [0, 0],
            ],
          ],
        },
        {
          type: 'Polygon',
          properties: { ISO_A2: 'CN', NAME: 'China' },
          arcs: [
            [
              [2, 2],
              [3, 2],
              [3, 3],
              [2, 3],
              [2, 2],
            ],
          ],
        },
      ],
    },
  },
  arcs: [
    [
      [0, 0],
      [1, 0],
      [1, 1],
      [0, 1],
      [0, 0],
    ],
    [
      [2, 2],
      [3, 2],
      [3, 3],
      [2, 3],
      [2, 2],
    ],
  ],
});

/**
 * Creates a mock country data for testing
 */
export const createMockCountryData = () => [
  {
    id: 'test-1',
    name: 'United States',
    isoAlpha2: 'US',
    isoAlpha3: 'USA',
    latitude: 39.8283,
    longitude: -98.5795,
    economicIndicators: [
      {
        name: 'GDP',
        value: 25462700000000,
        unit: 'USD',
        year: 2023,
        source: 'World Bank',
      },
    ],
  },
  {
    id: 'test-2',
    name: 'China',
    isoAlpha2: 'CN',
    isoAlpha3: 'CHN',
    latitude: 35.8617,
    longitude: 104.1954,
    economicIndicators: [
      {
        name: 'GDP',
        value: 17963171000000,
        unit: 'USD',
        year: 2023,
        source: 'World Bank',
      },
    ],
  },
];
