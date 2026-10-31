import * as patched from 'minimatch-patched';
export * from 'minimatch-patched';
type LegacyTest = { test(value: string): boolean; [Symbol.match](value: string): string[] | null };
export declare class Minimatch extends patched.Minimatch {
  makeRe(): LegacyTest;
}
export declare function makeRe(pattern: string, options?: patched.MinimatchOptions): LegacyTest;
declare const compatibility: Omit<typeof patched, 'Minimatch' | 'minimatch'> &
  Omit<typeof patched.minimatch, 'Minimatch'> & {
    (...args: Parameters<typeof patched.minimatch>): boolean;
    Minimatch: typeof Minimatch;
    minimatch: typeof patched.minimatch;
    makeRe: typeof makeRe;
  };
export declare const minimatch: typeof compatibility;
export default compatibility;
