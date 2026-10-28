# Global Analysis Technical Architecture

## Overview

The Global Analysis page at `/global` compares countries through a choropleth of one World Bank World Development Indicator (WDI). React owns the indicator and display choices, TanStack Query owns fetched data, and D3 draws and transforms the SVG. Keeping those responsibilities separate lets indicator changes recolor existing country paths while the tooltip and table expose the underlying values.

This reference describes the routed world map. [GlobalAnalysis](../../frontend/src/pages/GlobalAnalysis.tsx) shows only the **World Map** tab in release builds. Comparison, events and impact tabs are gated by `__FLAGS__.global_analysis_tabs`; the first two load separate components lazily, and impact is a placeholder. See [build flags](../../frontend/docs/build-flags.md) for the development profile.

## Contents

- [System and component architecture](#system-and-component-architecture)
- [State and data flow](#state-and-data-flow)
- [Data contracts](#data-contracts)
- [D3 integration](#d3-integration)
- [Rendering lifecycle](#rendering-lifecycle)
- [Testing boundaries](#testing-boundaries)
- [Accessibility](#accessibility)

## System and component architecture

The frontend uses React and TypeScript, MUI controls, D3 v7, `topojson-client`, and TanStack Query. Vite builds the application; Vitest with React Testing Library covers components and hooks, and Playwright covers browser interactions. The [package manifest](../../frontend/package.json) and [Vite configuration](../../frontend/vite.config.ts) define the toolchain.

```text
GlobalAnalysis (/global)
└── WorldMapTab
    ├── Indicator picker
    ├── WorldMapControls (projection and color scheme)
    ├── InteractiveWorldMap
    │   └── WorldMapContent (D3 SVG and zoom controls)
    │       └── CountryTooltip
    ├── MapValuesTable
    └── MapLegend
```

[WorldMapTab](../../frontend/src/components/global/WorldMapTab.tsx) coordinates the queries, selected indicator, projection and color scheme. It derives a shared color scale and passes it to the map and legend, so they describe the same values. It also turns a country click into navigation to `/series/{seriesId}`.

[InteractiveWorldMap](../../frontend/src/components/global/InteractiveWorldMap.tsx) loads the outline and mounts `WorldMapContent` once it is available. The content component owns the SVG, pointer hover state, touch tap tracking, and zoom buttons. D3 converts the TopoJSON into country paths and a border mesh. The drawing uses a 960 × 500 coordinate space supplied by the tab and scales down to fit narrower containers.

[WorldMapControls](../../frontend/src/components/global/WorldMapControls.tsx) provides the projection and color pickers. Zoom controls belong to the map: they sit over its top-right corner on larger screens and in a bar below it on phones, where an overlay would obscure countries.

[CountryTooltip](../../frontend/src/components/global/CountryTooltip.tsx) shows the hovered country's name, value, unit and observation date, or **No data**. It chooses its position relative to the pointer and map edges. [MapLegend](../../frontend/src/components/global/MapLegend.tsx) explains the selected indicator's scale, unit, missing-data fill, drawable country count and date range. It also counts values with no corresponding shape and directs readers to the table.

[MapValuesTable](../../frontend/src/components/global/MapValuesTable.tsx) is a collapsed table of every returned country value, sorted highest first, with country links to the series. Unlike the map, it includes countries that cannot be drawn. Its rows render in a scroll container when expanded; they are not virtualized.

## State and data flow

The map uses local React state rather than a Global Analysis Context provider. `WorldMapTab` owns indicator, projection and color choices; the page owns its active tab. `WorldMapContent` owns hover state, while [useWorldMap](../../frontend/src/components/global/hooks/useWorldMap.ts) owns the projection, path generator, zoom behavior and zoom status. D3 applies the zoom transform directly to `.map-container`.

### WDI queries

[useWorldMapData](../../frontend/src/components/global/hooks/useWorldMapData.ts) contains the dataset and cross-section queries:

1. `useMapDataset()` reads `datasets`, selects code `wdi`, and finds the area dimension through its `countries` code list. It accepts a dataset with exactly one other dimension, whose codes populate the indicator picker. The initial indicator is GDP per capita (`NY.GDP.PCAP.CD`) when available, otherwise the first code.
2. `useMapCrossSection(dataset, indicator)` filters that indicator dimension and queries across the area dimension with `latest: true`. Each country therefore has its own latest observation date; the result is not a common-year snapshot.
3. The hook keeps entries whose area kind is `COUNTRY`, with non-null values and dates and finite numeric conversions. Aggregates, unknown areas and missing observations do not become country values.
4. For a nonempty result, it queries the first country's series for units and frequency, treating those as shared by all series of the indicator. The indicator code's unit takes precedence over the series unit. If metadata lookup fails, values remain usable, the code unit is retained if known, and dates use their full-date format. Cancellation still stops the query.

The dataset query uses `['worldMap', 'dataset']` with a 30-minute stale time. The cross-section query uses `['worldMap', 'crossSection', dataset?.id, indicator]` with a five-minute stale time and runs only when both dataset and indicator are known. Both pass the query cancellation signal to the GraphQL client.

On an indicator switch, `keepPreviousData` preserves the preceding result while the next query loads. The map, legend and table take their indicator identity from that result, rather than relabeling old values with the picker's new choice. A progress bar shows fetching, and query failures offer retry actions. Missing WDI metadata and an empty drawable result have separate informational messages.

```text
WDI metadata → indicator choice → latest cross-section → country values
                                                        ├→ values table
Bundled atlas → drawable ISO numeric IDs ────────────────┴→ useCountryData
                                                            ├→ map fills
                                                            └→ legend ranges/counts
```

### Outline loading

[worldAtlas](../../frontend/src/components/global/worldAtlas.ts) dynamically imports `world-atlas/countries-50m.json` from the pinned package. Vite bundles this 1:50m TopoJSON as a separate chunk loaded when the map is used. The outline does not come from a third-party CDN. The finer outline includes small countries omitted by the package's 1:110m file, though some microstates and territories still have no shape.

The tab and map share `worldAtlasQuery` under the `['world-atlas']` key. Its stale time and garbage-collection time are infinite, so both consumers reuse the loaded outline. A load failure is retried once, then displayed in place of the map with a **Reload** action; a missing application chunk may require a page reload.

## Data contracts

The routed map's value type is [MapCountryValue](../../frontend/src/components/global/hooks/useWorldMapData.ts):

```typescript
interface MapCountryValue {
  key: string;                 // Area key, ISO alpha-3
  name: string;
  isoNumeric: number | null;   // Join key for world-atlas features
  seriesId: string;
  date: string;                // YYYY-MM-DD
  value: string;               // API decimal for display
  numericValue: number;        // Number used for color and sorting
}
```

Country identity has two roles: the API's area key identifies the observation, while ISO numeric code joins it to a world-atlas feature. Feature IDs such as `"040"` are converted to numbers before lookup. There is deliberately no fallback join by country name. Kosovo has no ISO numeric code; other countries can have a code but no shape in the atlas. Their values stay in the table and are excluded from map scale bounds, date ranges and drawable counts once the outline is known.

[useCountryData](../../frontend/src/components/global/hooks/useCountryData.ts) accepts the values, a color scheme (default `viridis`) and optional drawable IDs:

```typescript
const useCountryData = (
  countries: MapCountryValue[],
  colorScheme: string = 'viridis',
  drawableIds?: ReadonlySet<number>
) => {
  // Derive the numeric index, scale, ranges and drawable-country lists.
};
```

It returns `valuesByIsoNumeric`, `colorScale`, `dataRange`, `dateRange`, `countriesWithData` and `countriesWithoutData`. Until drawable IDs are available, any finite value with a numeric code is considered drawable. A shape without a joined value receives the gray `NO_DATA_FILL`, shows **No data** in its tooltip and does not open a series on click.

The decimal string and number serve different purposes. [mapFormat](../../frontend/src/components/global/mapFormat.ts) formats the string through decimal arithmetic, grouping thousands and rounding to at most two decimal places without trailing zeros. It avoids binary-number conversion for displayed values; this does not mean that every original decimal digit is displayed. Color interpolation and table sorting use `numericValue`. Date formatting uses series frequency to show annual, quarterly or monthly periods, with full dates when frequency is unknown.

The map's required inputs are `valuesByIsoNumeric: ReadonlyMap<number, MapCountryValue>`, `colorScale: (value: number) => string`, `width: number` and `height: number`. Optional props provide unit, frequency, `onCountryClick`, projection, borders and label settings. See [InteractiveWorldMapProps](../../frontend/src/components/global/InteractiveWorldMap.tsx) for the full contract; the component does not accept an indicator or time-range filter.

## D3 integration

### Projection and zoom

`useWorldMap(svgRef, projectionType = 'naturalEarth', size?)` returns `projection`, `path`, `zoomBehavior`, `zoomLevel`, `isTransformed`, `zoomBy`, `resetZoom` and the projection keys. Each map instance receives a fresh projection. With a known drawing size, the hook fits the projection to a GeoJSON sphere; without one, it refits on window resize using the SVG bounds.

The selector keys differ from the D3 constructor names:

| Application key | D3 constructor | View |
| --- | --- | --- |
| `naturalEarth` | `geoNaturalEarth1` | Balanced world overview, the default |
| `mercator` | `geoMercator` | Rectangular world view |
| `orthographic` | `geoOrthographic` | Globe view |

Unknown projection keys fall back to Natural Earth. Orthographic is a projection choice, not a globe-rotation control.

D3 zoom uses `scaleExtent([1, 8])`: scale 1 is the fitted whole-world view and scale 8 is the maximum magnification. When dimensions are known, `translateExtent` bounds panning to the drawing area. Zoom events transform `.map-container` and update the displayed zoom percentage and reset availability. Buttons zoom by a factor of 1.5 or its inverse; reset applies `zoomIdentity`. Rebuilding geometry preserves the SVG's current zoom transform.

### Color scale

`useCountryData` derives the minimum and maximum from drawable numeric values and uses a sequential interpolator:

```typescript
const interpolator = COLOR_SCHEMES[colorScheme as ColorScheme] || interpolateViridis;
return scaleSequential(interpolator).domain([dataRange.min, dataRange.max]);
```

The available schemes are Viridis, Blues, Reds and Greens. Viridis supplies a perceptually uniform sequential scale; the single-hue schemes offer alternative presentations of the same domain. With no drawable values, the range falls back to `[0, 1]` and the legend omits the value gradient. The legend's date range likewise describes drawable observations, not all entries in the table.

## Rendering lifecycle

The map separates geometry construction from data styling. Its construction effect converts TopoJSON to features, generates paths and borders, adds optional labels, and installs pointer and click handlers. It runs when the outline, path generator, dimensions, border/label settings or zoom behavior change. A second effect updates fills and cursors when values or the color scale change, retaining existing country nodes.

Handlers read current values and the click callback through a ref updated in a layout effect. Changing indicators therefore does not require rebuilding handlers. Hover changes update tooltip state and the hovered path's stroke without reconstructing map geometry. A mouse click on a country with data opens its series; on touch screens, the first tap exposes the tooltip and the second opens the series.

These mechanisms avoid repeated geometry work during indicator and pointer interactions. They describe the implemented lifecycle, not measured load-time, interaction-latency or memory guarantees.

## Testing boundaries

The active [Vitest configuration](../../frontend/vitest.config.ts) uses jsdom and [setupTests.vitest.ts](../../frontend/src/setupTests.vitest.ts), with D3 dependencies included in its dependency handling. The shared setup stubs some D3 modules; tests needing real paths explicitly unmock them.

- [WorldMapTab.test.tsx](../../frontend/src/components/global/__tests__/WorldMapTab.test.tsx) uses mocked GraphQL with real map paths to exercise indicator loading, fills, tooltip formatting, series navigation, touch taps, table values and errors. It also asserts that hover and indicator changes retain country nodes.
- [Hook tests](../../frontend/src/components/global/hooks/__tests__) exercise dataset selection, country filtering, ISO numeric indexing, ranges, color scales, projection fitting and zoom bounds.
- [worldAtlas.test.tsx](../../frontend/src/components/global/__tests__/worldAtlas.test.tsx) checks package loading and outline rendering without fetching. [InteractiveWorldMap.error.test.tsx](../../frontend/src/components/global/__tests__/InteractiveWorldMap.error.test.tsx) exercises the outline failure state.

Browser tests live under [release/world-map](../../frontend/tests/e2e/release/world-map). `world-map-mocked.spec.ts` intercepts GraphQL to check UI behavior, including aggregate exclusion and missing shapes. `world-atlas.spec.ts` checks bundled loading and an aborted outline load. The [standalone Global Analysis configuration](../../frontend/ci/configs/playwright-global-analysis.config.ts) excludes `world-map.spec.ts`, which requires the seeded release backend.

`world-map.spec.ts` checks seeded countries across indicators, displayed values and dates, tooltips and outline requests. `world-map-every-country.spec.ts` derives expectations from recorded World Bank responses for fixture mode and checks seeded values and no-data outlines after indicator changes. Its seed omits aggregates, so the mocked spec supplies coverage of the frontend aggregate filter. These layers separate UI behavior under controlled responses from integration with the release fixture stack; they do not establish live World Bank coverage, performance budgets or accessibility conformance.

## Accessibility

The SVG has `role="img"` and `aria-label="World map"`. Indicator, projection and color selectors have labels; zoom buttons have accessible names, and the zoom percentage uses a polite live region. These controls provide keyboard access to display choices and zoom.

Individual country paths use pointer handlers and are not keyboard-focusable. The expandable **Values as a table** provides an alternative route to country data and series navigation through ordinary links, with country, value and date columns and a labeled table. It includes returned values without drawable shapes, so data exploration does not depend entirely on color, hover or map geometry. This implementation does not establish WCAG AA conformance or full keyboard operation of the SVG countries.
