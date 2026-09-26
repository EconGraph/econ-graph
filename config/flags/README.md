# Feature flags

The flag files for EconGraph, in [flagd](https://flagd.dev)'s flag format. The
design, including which kinds of flag exist and when not to use one, is in
[docs/roadmap/feature-flags.md](../../docs/roadmap/feature-flags.md).

| File | What it holds |
|---|---|
| `flags.flagd.json` | Every flag. Each flag's `defaultVariant` is its **release** value |
| `dev.flagd.json` | Overrides for development builds and local runs, and nothing else |
| `schema/` | flagd's JSON schema, vendored so the check needs no network |

A consumer reads `flags.flagd.json`, then, for the `dev` profile, replaces each
flag that `dev.flagd.json` names with the override. This is flagd's own rule for
several sources: a later source's flag replaces the earlier one whole.

## Adding a flag

1. Add it to `flags.flagd.json`, with a camelCase key (it becomes a constant
   such as `FLAGS.worldMap` in the frontend), `"state": "ENABLED"`, boolean
   `on`/`off` variants, and the release value as `defaultVariant`.
2. Give it `metadata`:

   | Key | Required for | Value |
   |---|---|---|
   | `kind` | every flag | `build` (unfinished code compiled out of release builds), `preview` (works end to end, shown to some people first), `ops` (kill switch), or `experiment` |
   | `owner` | every flag | The roadmap doc that owns it, such as `docs/roadmap/feature-flags.md` |
   | `stage` | `preview` only | `alpha` or `beta` |
   | `remove_by` | `build` and `preview` | `"train N"` or `"unscheduled"`. CI fails once train N's tag, `v0.(N+1).0`, exists and the flag is still here |

   `ops` flags are long-lived and take no `remove_by`. `build` flags are folded
   into the bundle at build time, so they have boolean variants and no
   `targeting`.
3. To change its value for development, add an override to `dev.flagd.json`:
   the same `state` and `variants`, a different `defaultVariant`, and no
   `metadata`. Leave it out when development should match release, which is the
   case for code parked behind a flag that is off everywhere.
4. Run the check:

   ```sh
   node scripts/check-flags
   (cd scripts/check-flags && npm test)
   ```

Example:

```json
"worldMap": {
  "state": "ENABLED",
  "variants": { "on": true, "off": false },
  "defaultVariant": "off",
  "metadata": {
    "kind": "build",
    "owner": "docs/roadmap/global-analysis.md",
    "remove_by": "train 1"
  }
}
```

## Removing a flag

Delete it from both files in the PR that finishes or replaces the feature, and
remove the code paths for its other value.

## The seeded flags

- `buildCanary` (build, off in release, on in dev) guards a module with a unique
  marker string, so CI can prove a release bundle leaves flagged-off code out.
- `mcp` (ops, off in release, on in dev) decides whether the backend routes
  `/mcp`. MCP ships in train 2.

## Updating the vendored schema

`schema/flags.json` and `schema/targeting.json` are copied unchanged from
[open-feature/flagd-schemas](https://github.com/open-feature/flagd-schemas)
at tag `json/json-schema-v0.2.15` (Apache-2.0, see `schema/LICENSE`). To update,
copy both files from a newer tag's `json/` directory and run the check. The
check's validator supports only the JSON Schema keywords these files use, and
fails loudly on any other.
