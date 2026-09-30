/**
 * Format a decimal number given as a string, without converting it to a float.
 *
 * The backend serializes `BigDecimal` values as strings. Parsing them with `Number` would
 * round them to binary floating point, so this works on the digits instead: it rounds half
 * away from zero to a fixed number of fraction digits and groups the integer part in
 * thousands with commas.
 */

const DECIMAL_PATTERN = /^([+-])?(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/;

/**
 * Round and format a decimal string.
 * @param value - A decimal such as "27360.935", "-0.5" or "1.2E+3".
 * @param fractionDigits - Digits to keep after the decimal point.
 * @returns The formatted number, for example "27,360.9", or null when `value` is not a
 * decimal number.
 */
export function formatDecimalString(value: string, fractionDigits: number): string | null {
  const match = DECIMAL_PATTERN.exec(value.trim());
  if (!match) return null;
  const [, sign, intDigits = '', fracDigits = '', exponentText = '0'] = match;
  if (intDigits === '' && fracDigits === '') return null;

  // value = coefficient * 10^-scale
  let coefficient = BigInt(`${intDigits}${fracDigits}` || '0');
  const scale = fracDigits.length - Number(exponentText);
  if (!Number.isSafeInteger(scale) || Math.abs(scale) > 1000) return null;

  if (scale > fractionDigits) {
    const divisor = 10n ** BigInt(scale - fractionDigits);
    const remainder = coefficient % divisor;
    coefficient /= divisor;
    if (remainder * 2n >= divisor) coefficient += 1n;
  } else if (scale < fractionDigits) {
    coefficient *= 10n ** BigInt(fractionDigits - scale);
  }

  const digits = coefficient.toString().padStart(fractionDigits + 1, '0');
  const integerPart = digits.slice(0, digits.length - fractionDigits);
  const fractionPart = digits.slice(digits.length - fractionDigits);
  const grouped = integerPart.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  const negative = sign === '-' && coefficient !== 0n;

  return `${negative ? '-' : ''}${grouped}${fractionDigits > 0 ? `.${fractionPart}` : ''}`;
}
