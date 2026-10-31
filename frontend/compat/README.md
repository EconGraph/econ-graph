# Patched minimatch compatibility

The frontend still uses ESLint plugins and glob 7 consumers that require the
callable minimatch 3 interface. Their current published versions cannot consume
minimatch 11 directly. The local `minimatch` package adapts those interfaces to
registry `minimatch@11.0.1`, which fixes GHSA-g3hv-75wg-m249. It contains no
glob parser, regular-expression generator, or matching implementation.

The adapter preserves callable CommonJS/default imports and modern named
exports. For glob 7 it exposes the patched matcher's pattern metadata and
`Symbol.match`; for eslint-plugin-import it supplies `makeRe(...).test(...)`
through the patched matcher. This is compatibility for the repository's tested
consumers, not a complete replacement for the old RegExp API: consumers needing
`makeRe().source` or other RegExp internals must migrate before using it. Remove
the adapter when these upstream dependencies support the patched API.

Modern glob consumers use 14.0.2 to fix GHSA-3g3m-2fjr-6pw4. Glob 7 retains its
callback API; that advisory's affected range begins at glob 9, and all its
minimatch matching is delegated to the patched implementation. Both selected
registry versions support the pinned Node 26 runtime.

The patched implementation remains an integrity-pinned registry dependency named
`minimatch-patched` (`npm:minimatch@11.0.1`), visible to npm audit under the
real package name. A negative-control audit with this alias pinned to vulnerable
10.2.6 reports GHSA-g3hv-75wg-m249. The mandatory audit helper checks the
adapter's reviewed source hashes, exact exports/dependency, and lock routing
before running npm audit; it rejects added scripts, substituted implementations,
and old nested minimatch packages. No advisory allowlist entry is added.

Run `npm run test:dependency-compat` to exercise real glob 7 sync/callback and
glob 14 Promise APIs, the ESLint import rule, module exports, and guard
rejection cases. The Docker build contexts copy the adapter before `npm ci`
resolves the local package link.
