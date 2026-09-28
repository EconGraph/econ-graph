/**
 * World Atlas Loader.
 *
 * Loads the Natural Earth 1:110m country outlines from the pinned `world-atlas`
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
 * Load the 1:110m countries topology bundled with the app.
 * @returns The world-atlas countries topology.
 */
export const loadWorldAtlas = async (): Promise<WorldAtlasTopology> => {
  const { default: atlas } = await import('world-atlas/countries-110m.json');
  // TypeScript infers `arcs` as number[][][], which doesn't satisfy topojson's Arc type.
  return atlas as unknown as WorldAtlasTopology;
};
