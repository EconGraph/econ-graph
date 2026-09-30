# Build-time feature flags

Build-time flags compile code out of the frontend bundle. They implement phase 1
of [the feature flags roadmap](../../docs/roadmap/feature-flags.md): unfinished
or unscheduled screens are absent from a release build, not hidden at runtime.

## Where the values come from

Flags are defined in `config/flags/` at the repository root
([README](../../config/flags/README.md)). `node scripts/flags-merge` resolves
them into `frontend/flags/release.json` and `frontend/flags/dev.json`, which are
committed so the frontend Docker build can see them.

`flags.config.ts` reads the file for one profile and gives Vite a `define` entry
per `kind: "build"` flag, `__FLAGS__.<key>` (`__FLAGS__.world_map`), set to
`true` or `false`. Each flag is its own dotted `define` key; a whole `__FLAGS__`
object would not be folded, so flagged code would stay in the bundle.

| Command                           | Profile   |
| --------------------------------- | --------- |
| `vite build` (`npm run build`)    | `release` |
| `vite` (dev server), `vitest`     | `dev`     |
| any of these with `FLAGS_PROFILE` | its value |

`FLAGS_PROFILE` must be `release` or `dev`. The Docker image takes it as a build
argument, `ARG FLAGS_PROFILE=release`. The Vite config fails to load when the
profile's file is missing, is for another profile, or has a build flag that is
not a boolean.

## Using a flag

1. Add the flag to `config/flags/` as its README says, then run
   `node scripts/flags-merge`. `src/flags.ts` takes the flag names from the
   generated `flags/release.json` (a type-only import), so the new name
   typechecks and an unknown name does not.
2. Read it as `__FLAGS__.<key>` in the condition that guards the code, and load
   the guarded code with `React.lazy` behind it:

<!-- prettier-ignore -->
```tsx
const WorldMap = __FLAGS__.world_map ? lazy(() => import('./pages/WorldMap')) : null;

// In the routes:
{WorldMap && (
  <Route
    path='/world'
    element={
      <Suspense fallback={null}>
        <WorldMap />
      </Suspense>
    }
  />
)}

// In a menu:
{__FLAGS__.world_map && <MenuItem onClick={() => navigate('/world')}>World map</MenuItem>}
```

Vite replaces `__FLAGS__.world_map` with `false` in a release build, so the
bundler drops the branch, and with it the only `import()` of the page. The
page's module, and anything only it imports, is left out of the bundle. The
guard itself (the route path and the `import()` path) stays readable in the
served sourcemaps.

Rules:

- Import flagged code only with `import()`, behind the flag itself. A static
  `import` at the top of a file is bundled whatever the flag says, and so is a
  module-level `const Page = lazy(() => import('./X'))` even when every use of
  `Page` is behind a false flag: `lazy()` is not known to be pure, so X's chunk
  is still emitted.
- Read a flag only as `__FLAGS__.<key>`, in the condition itself. `__FLAGS__` on
  its own is not defined in a build (the dev server does define it, so misuse
  shows up only in a build), and a flag copied into a variable, object or prop
  may not be folded, which keeps the flagged code in the bundle. Lint in
  `eslint.config.js` rejects a bare or computed `__FLAGS__`.
- Never read the flag files at runtime (`config/flags`, `frontend/flags`). Lint
  in `eslint.config.js` forbids importing or naming them in `src/`. Runtime
  flags are train 2 (OFREP served by the backend).

Only `kind: "build"` flags are defined. The type in `src/flags.ts` lists every
flag in the generated file, which in train 1 are all build flags; runtime flags
(train 2) get their own API rather than `__FLAGS__`. The build fails if its
output still reads `__FLAGS__`, which catches a mistyped or non-build flag even
where `tsc` did not run.

## The canary check

The `build_canary` flag is off in release and on in dev. It guards
`/flags/canary` (`src/pages/FlagCanary.tsx`), which renders a unique marker
string. `npm run check:flag-bundle` builds both profiles, scans every file each
emits (sourcemaps included), and fails when the release output contains the
marker or the dev output lacks it. CI runs it in the Frontend Tests job.
