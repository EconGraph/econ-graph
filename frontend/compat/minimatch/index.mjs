import compatibility from './index.cjs';
export * from 'minimatch-patched';
export const Minimatch = compatibility.Minimatch;
export const minimatch = compatibility;
export const makeRe = compatibility.makeRe;
export default compatibility;
