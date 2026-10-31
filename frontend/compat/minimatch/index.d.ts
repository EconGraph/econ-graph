import * as patched from 'minimatch-patched';
export * from 'minimatch-patched';
type LegacyTest = { test(value: string): boolean; [Symbol.match](value: string): string[] | null };
declare const compatibility: typeof patched.minimatch &
  typeof patched & {
    makeRe(pattern: string, options?: Parameters<typeof patched.minimatch>[2]): LegacyTest;
  };
export default compatibility;
