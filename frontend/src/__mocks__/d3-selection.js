/**
 * Mock implementation of D3-Selection for testing
 * PURPOSE: Provide predictable test behavior for D3 selection components
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
            text: vi.fn(() => ({})),
          })),
        })),
        exit: vi.fn(() => ({
          remove: vi.fn(() => ({})),
        })),
      })),
    })),
    attr: vi.fn(() => ({})),
    style: vi.fn(() => ({})),
    text: vi.fn(() => ({})),
    append: vi.fn(() => ({
      attr: vi.fn(() => ({})),
      style: vi.fn(() => ({})),
      text: vi.fn(() => ({})),
    })),
    remove: vi.fn(() => ({})),
    on: vi.fn(() => ({})),
    call: vi.fn(() => ({})),
  })),
  selectAll: vi.fn(() => ({
    data: vi.fn(() => ({
      enter: vi.fn(() => ({
        append: vi.fn(() => ({
          attr: vi.fn(() => ({
            style: vi.fn(() => ({})),
          })),
        })),
      })),
    })),
    attr: vi.fn(() => ({})),
    style: vi.fn(() => ({})),
    on: vi.fn(() => ({})),
  })),
};
