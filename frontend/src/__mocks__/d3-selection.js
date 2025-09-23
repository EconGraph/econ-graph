/**
 * Mock implementation of D3-Selection for testing
 * PURPOSE: Provide predictable test behavior for D3 selection components
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
            text: jest.fn(() => ({})),
          })),
        })),
        exit: jest.fn(() => ({
          remove: jest.fn(() => ({})),
        })),
      })),
    })),
    attr: jest.fn(() => ({})),
    style: jest.fn(() => ({})),
    text: jest.fn(() => ({})),
    append: jest.fn(() => ({
      attr: jest.fn(() => ({})),
      style: jest.fn(() => ({})),
      text: jest.fn(() => ({})),
    })),
    remove: jest.fn(() => ({})),
    on: jest.fn(() => ({})),
    call: jest.fn(() => ({})),
  })),
  selectAll: jest.fn(() => ({
    data: jest.fn(() => ({
      enter: jest.fn(() => ({
        append: jest.fn(() => ({
          attr: jest.fn(() => ({
            style: jest.fn(() => ({})),
          })),
        })),
      })),
    })),
    attr: jest.fn(() => ({})),
    style: jest.fn(() => ({})),
    on: jest.fn(() => ({})),
  })),
};
