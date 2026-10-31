'use strict';
const patched = require('minimatch-patched');

// glob7 consumes these two RegExp-like interfaces from Minimatch.set. The
// patched matcher owns all parsing and matching; this only exposes its result.
class Minimatch extends patched.Minimatch {
  constructor(...args) {
    super(...args);
    for (const row of this.set) {
      for (const part of row) {
        if (typeof part === 'object' && part !== null && typeof part.test === 'function') {
          part._glob = part.pattern;
          part[Symbol.match] = value => (part.test(value) ? [value] : null);
        }
      }
    }
  }

  // eslint-plugin-import consumes only the test interface, not regex source.
  makeRe() {
    return {
      test: value => this.match(value),
      [Symbol.match]: value => (this.match(value) ? [value] : null),
    };
  }
}

const minimatch = (...args) => patched.minimatch(...args);
const makeRe = (...args) => new Minimatch(...args).makeRe();
module.exports = Object.assign(minimatch, patched, { Minimatch, makeRe });
