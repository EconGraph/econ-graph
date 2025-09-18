/**
 * Mock for d3-selection module
 * Provides mock implementations for DOM selection functions
 */

// Mock select function
export const select = jest.fn(() => ({
  select: jest.fn().mockReturnThis(),
  selectAll: jest.fn().mockReturnThis(),
  append: jest.fn().mockReturnThis(),
  insert: jest.fn().mockReturnThis(),
  remove: jest.fn().mockReturnThis(),
  attr: jest.fn().mockReturnThis(),
  style: jest.fn().mockReturnThis(),
  classed: jest.fn().mockReturnThis(),
  text: jest.fn().mockReturnThis(),
  html: jest.fn().mockReturnThis(),
  property: jest.fn().mockReturnThis(),
  datum: jest.fn().mockReturnThis(),
  data: jest.fn().mockReturnThis(),
  enter: jest.fn().mockReturnThis(),
  exit: jest.fn().mockReturnThis(),
  merge: jest.fn().mockReturnThis(),
  order: jest.fn().mockReturnThis(),
  sort: jest.fn().mockReturnThis(),
  call: jest.fn().mockReturnThis(),
  each: jest.fn().mockReturnThis(),
  on: jest.fn().mockReturnThis(),
  dispatch: jest.fn().mockReturnThis(),
  filter: jest.fn().mockReturnThis(),
  node: jest.fn().mockReturnValue(document.createElement('div')),
  nodes: jest.fn().mockReturnValue([document.createElement('div')]),
  size: jest.fn().mockReturnValue(1),
  empty: jest.fn().mockReturnValue(false),
}));

// Mock selectAll function
export const selectAll = jest.fn(() => ({
  select: jest.fn().mockReturnThis(),
  selectAll: jest.fn().mockReturnThis(),
  append: jest.fn().mockReturnThis(),
  insert: jest.fn().mockReturnThis(),
  remove: jest.fn().mockReturnThis(),
  attr: jest.fn().mockReturnThis(),
  style: jest.fn().mockReturnThis(),
  classed: jest.fn().mockReturnThis(),
  text: jest.fn().mockReturnThis(),
  html: jest.fn().mockReturnThis(),
  property: jest.fn().mockReturnThis(),
  datum: jest.fn().mockReturnThis(),
  data: jest.fn().mockReturnThis(),
  enter: jest.fn().mockReturnThis(),
  exit: jest.fn().mockReturnThis(),
  merge: jest.fn().mockReturnThis(),
  order: jest.fn().mockReturnThis(),
  sort: jest.fn().mockReturnThis(),
  call: jest.fn().mockReturnThis(),
  each: jest.fn().mockReturnThis(),
  on: jest.fn().mockReturnThis(),
  dispatch: jest.fn().mockReturnThis(),
  filter: jest.fn().mockReturnThis(),
  node: jest.fn().mockReturnValue(document.createElement('div')),
  nodes: jest.fn().mockReturnValue([document.createElement('div')]),
  size: jest.fn().mockReturnValue(1),
  empty: jest.fn().mockReturnValue(false),
}));

// Mock create function
export const create = jest.fn().mockReturnValue({
  select: jest.fn().mockReturnThis(),
  selectAll: jest.fn().mockReturnThis(),
  append: jest.fn().mockReturnThis(),
  insert: jest.fn().mockReturnThis(),
  remove: jest.fn().mockReturnThis(),
  attr: jest.fn().mockReturnThis(),
  style: jest.fn().mockReturnThis(),
  classed: jest.fn().mockReturnThis(),
  text: jest.fn().mockReturnThis(),
  html: jest.fn().mockReturnThis(),
  property: jest.fn().mockReturnThis(),
  datum: jest.fn().mockReturnThis(),
  data: jest.fn().mockReturnThis(),
  enter: jest.fn().mockReturnThis(),
  exit: jest.fn().mockReturnThis(),
  merge: jest.fn().mockReturnThis(),
  order: jest.fn().mockReturnThis(),
  sort: jest.fn().mockReturnThis(),
  call: jest.fn().mockReturnThis(),
  each: jest.fn().mockReturnThis(),
  on: jest.fn().mockReturnThis(),
  dispatch: jest.fn().mockReturnThis(),
  filter: jest.fn().mockReturnThis(),
  node: jest.fn().mockReturnValue(document.createElement('div')),
  nodes: jest.fn().mockReturnValue([document.createElement('div')]),
  size: jest.fn().mockReturnValue(1),
  empty: jest.fn().mockReturnValue(false),
});

// Mock event function
export const event = jest.fn().mockReturnValue({
  type: 'click',
  target: document.createElement('div'),
  currentTarget: document.createElement('div'),
  preventDefault: jest.fn(),
  stopPropagation: jest.fn(),
});

// Mock mouse function
export const mouse = jest.fn().mockReturnValue([100, 200]);

// Mock touch function
export const touch = jest.fn().mockReturnValue([{ identifier: 0, x: 100, y: 200 }]);

// Mock touches function
export const touches = jest.fn().mockReturnValue([{ identifier: 0, x: 100, y: 200 }]);

// Default export
export default {
  select,
  selectAll,
  create,
  event,
  mouse,
  touch,
  touches,
};
