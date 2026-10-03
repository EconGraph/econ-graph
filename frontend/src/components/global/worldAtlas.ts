/**
 * World Atlas Loader.
 *
 * Loads the Natural Earth 1:50m country outlines from the pinned `world-atlas`
 * package. Vite bundles the TopoJSON into its own chunk, so the map never
 * fetches its outline from a third-party CDN. The dynamic import keeps the
 * outline out of the main bundle until a map is rendered.
 */

import type { Topology, GeometryCollection } from 'topojson-specification';

/** Properties world-atlas attaches to each country geometry. */
export interface WorldAtlasCountryProperties {
  /** Natural Earth country name. */
  name: string;
}

/** The world-atlas countries topology. Geometry ids are ISO 3166-1 numeric codes. */
export type WorldAtlasTopology = Topology<{
  countries: GeometryCollection<WorldAtlasCountryProperties>;
  land: GeometryCollection;
}>;

/**
 * React-query options for the outline, shared by everything that reads it so they share one
 * load: it never changes, a load failure (such as a stale chunk after a deploy) is shown in place
 * of the map rather than thrown, and a failure is retried once, since a chunk that is really
 * missing needs a reload.
 */
export const worldAtlasQuery = {
  queryKey: ['world-atlas'],
  queryFn: () => loadWorldAtlas(),
  staleTime: Infinity,
  cacheTime: Infinity,
  useErrorBoundary: false,
  retry: 1,
};

/**
 * Load the 1:50m countries topology bundled with the app. The 1:110m file leaves out about 75
 * small countries and territories (Singapore, Hong Kong, Malta, Bahrain, Monaco...), whose
 * values the map would then have no shape for; 1:50m draws every country except a few
 * microstates and territories (Tuvalu, Gibraltar, Réunion...), which the values table lists.
 * @returns The world-atlas countries topology.
 */
export const loadWorldAtlas = async (): Promise<WorldAtlasTopology> => {
  const { default: atlas } = await import('world-atlas/countries-50m.json');
  // TypeScript infers `arcs` as number[][][], which doesn't satisfy topojson's Arc type.
  return atlas as unknown as WorldAtlasTopology;
};
