import { formatDecimalString } from '../formatDecimal';

describe('formatDecimalString', () => {
  test.each([
    ['29723.8649', 1, '29,723.9'],
    ['101.500000', 2, '101.50'],
    ['319.082500', 3, '319.083'],
    ['4.1', 1, '4.1'],
    ['4.33', 2, '4.33'],
    ['4', 2, '4.00'],
    ['319.0825', 3, '319.083'],
    ['0.125', 2, '0.13'],
    ['-0.125', 2, '-0.13'],
    ['-0.004', 2, '0.00'],
    ['0.5', 0, '1'],
    ['1234567.891', 0, '1,234,568'],
    ['999.95', 1, '1,000.0'],
    ['.5', 1, '0.5'],
    ['7.', 1, '7.0'],
    ['+12', 0, '12'],
    ['1.2E+3', 1, '1,200.0'],
    ['12345e-2', 2, '123.45'],
    ['123456789012345678901234567890.123', 2, '123,456,789,012,345,678,901,234,567,890.12'],
    ['  42.0 ', 1, '42.0'],
  ])('formats %s with %i fraction digits as %s', (value, digits, expected) => {
    expect(formatDecimalString(value, digits)).toBe(expected);
  });

  test.each([
    // BEA GDP is stored in dollars: 28,624,702 million dollars is 28.62 trillion.
    ['28624702000000', 2, 12, '28.62'],
    ['28624702000000.00', 1, 9, '28,624.7'],
    // Rounds half away from zero on the scaled digits, not on a float.
    ['28625000000000', 2, 12, '28.63'],
    ['-28625000000000', 2, 12, '-28.63'],
    ['1.2E+13', 2, 12, '12.00'],
    ['157808', 1, 3, '157.8'],
    ['499', 0, 3, '0'],
    ['500', 0, 3, '1'],
    ['4.1', 1, 0, '4.1'],
  ])('formats %s with %i fraction digits divided by 10^%i as %s', (value, digits, exp, expected) => {
    expect(formatDecimalString(value, digits, exp)).toBe(expected);
  });

  test.each(['', '.', '-', 'abc', '1.2.3', '1e', 'NaN', 'Infinity', '1e99999'])(
    'returns null for %j',
    value => {
      expect(formatDecimalString(value, 2)).toBeNull();
    }
  );
});
