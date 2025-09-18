/**
 * Mock for d3-scale-chromatic module
 * Provides mock implementations for color scale functions
 */

// Mock interpolateViridis function
export const interpolateViridis = jest.fn().mockReturnValue('#440154');

// Mock interpolatePlasma function
export const interpolatePlasma = jest.fn().mockReturnValue('#0d0887');

// Mock interpolateInferno function
export const interpolateInferno = jest.fn().mockReturnValue('#000004');

// Mock interpolateMagma function
export const interpolateMagma = jest.fn().mockReturnValue('#000004');

// Mock interpolateBlues function
export const interpolateBlues = jest.fn().mockReturnValue('#f7fbff');

// Mock interpolateGreens function
export const interpolateGreens = jest.fn().mockReturnValue('#f7fcf5');

// Mock interpolateReds function
export const interpolateReds = jest.fn().mockReturnValue('#fff5f0');

// Mock interpolateOranges function
export const interpolateOranges = jest.fn().mockReturnValue('#fff5eb');

// Mock interpolatePurples function
export const interpolatePurples = jest.fn().mockReturnValue('#fcfbfd');

// Mock interpolateGreys function
export const interpolateGreys = jest.fn().mockReturnValue('#ffffff');

// Mock schemeCategory10 function
export const schemeCategory10 = [
  '#1f77b4',
  '#ff7f0e',
  '#2ca02c',
  '#d62728',
  '#9467bd',
  '#8c564b',
  '#e377c2',
  '#7f7f7f',
  '#bcbd22',
  '#17becf',
];

// Mock schemeCategory20 function
export const schemeCategory20 = [
  '#1f77b4',
  '#aec7e8',
  '#ff7f0e',
  '#ffbb78',
  '#2ca02c',
  '#98df8a',
  '#d62728',
  '#ff9896',
  '#9467bd',
  '#c5b0d5',
  '#8c564b',
  '#c49c94',
  '#e377c2',
  '#f7b6d3',
  '#7f7f7f',
  '#c7c7c7',
  '#bcbd22',
  '#dbdb8d',
  '#17becf',
  '#9edae5',
];

// Mock schemeSet1 function
export const schemeSet1 = [
  '#e41a1c',
  '#377eb8',
  '#4daf4a',
  '#984ea3',
  '#ff7f00',
  '#ffff33',
  '#a65628',
  '#f781bf',
  '#999999',
];

// Mock schemeSet2 function
export const schemeSet2 = [
  '#66c2a5',
  '#fc8d62',
  '#8da0cb',
  '#e78ac3',
  '#a6d854',
  '#ffd92f',
  '#e5c494',
  '#b3b3b3',
];

// Mock schemeSet3 function
export const schemeSet3 = [
  '#8dd3c7',
  '#ffffb3',
  '#bebada',
  '#fb8072',
  '#80b1d3',
  '#fdb462',
  '#b3de69',
  '#fccde5',
  '#d9d9d9',
  '#bc80bd',
  '#ccebc5',
  '#ffed6f',
];

// Default export
export default {
  interpolateViridis,
  interpolatePlasma,
  interpolateInferno,
  interpolateMagma,
  interpolateBlues,
  interpolateGreens,
  interpolateReds,
  interpolateOranges,
  interpolatePurples,
  interpolateGreys,
  schemeCategory10,
  schemeCategory20,
  schemeSet1,
  schemeSet2,
  schemeSet3,
};
