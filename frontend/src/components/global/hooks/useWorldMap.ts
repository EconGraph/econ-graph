/**
 * UseWorldMap Hook.
 *
 * Custom hook for managing D3.js world map logic including projection,
 * path generation, zoom behavior, and responsive updates.
 */

import { useState, useEffect, useCallback } from 'react';
import * as d3 from 'd3';
import { geoPath, geoNaturalEarth1, geoMercator, geoOrthographic } from 'd3-geo';
import type { GeoProjection } from 'd3-geo';
import { zoom } from 'd3-zoom';
import type { ZoomBehavior } from 'd3-zoom';

export interface MapProjection {
  name: string;
  /** Builds a fresh projection, so map instances never share (and mutate) one. */
  create: () => GeoProjection;
  defaultScale: number;
  defaultCenter: [number, number];
}

/** Zoom scale limits: the whole world at most, eight times closer at least. */
export const MIN_ZOOM = 1;
export const MAX_ZOOM = 8;

export const PROJECTIONS: Record<string, MapProjection> = {
  naturalEarth: {
    name: 'Natural Earth',
    create: geoNaturalEarth1,
    defaultScale: 150,
    defaultCenter: [0, 0],
  },
  mercator: {
    name: 'Mercator',
    create: geoMercator,
    defaultScale: 100,
    defaultCenter: [0, 0],
  },
  orthographic: {
    name: 'Orthographic',
    create: geoOrthographic,
    defaultScale: 200,
    defaultCenter: [0, 0],
  },
};

/**
 * Build a projection of the given type, fitted to the whole globe when the
 * container size is known and at its default scale otherwise.
 * @param projectionType Key into PROJECTIONS; unknown types fall back to Natural Earth.
 * @param size Container width and height in pixels, when measured.
 * @returns A new projection.
 */
const buildProjection = (projectionType: string, size?: [number, number]): GeoProjection => {
  const proj = PROJECTIONS[projectionType] || PROJECTIONS.naturalEarth;
  const projection = proj.create();
  if (size && size[0] > 0 && size[1] > 0) {
    return projection.fitSize(size, { type: 'Sphere' });
  }
  return projection.scale(proj.defaultScale).center(proj.defaultCenter);
};

export const useWorldMap = (
  svgRef: React.RefObject<SVGSVGElement>,
  projectionType = 'naturalEarth',
  size?: [number, number]
) => {
  const [projection, setProjection] = useState(() => buildProjection(projectionType, size));

  const [path, setPath] = useState(() => geoPath().projection(projection));

  const [zoomBehavior, setZoomBehavior] = useState<ZoomBehavior<SVGSVGElement, unknown> | null>(
    null
  );

  // Current zoom scale, 1 when the whole world is in view
  const [zoomLevel, setZoomLevel] = useState(1);
  // Whether the map is zoomed or panned away from the whole-world view
  const [isTransformed, setIsTransformed] = useState(false);

  const [fitWidth, fitHeight] = size ?? [0, 0];

  // Projections, path generators and zoom behaviors are all functions, so every
  // setter below wraps them in an updater; React would otherwise call them as one.

  // Create zoom behavior
  useEffect(() => {
    if (!svgRef.current) return;

    const zoomBehavior = zoom<SVGSVGElement, unknown>()
      .scaleExtent([MIN_ZOOM, MAX_ZOOM])
      .on('zoom', event => {
        const { transform } = event;
        const mapContainer = d3.select(svgRef.current).select('.map-container');
        mapContainer.attr('transform', transform);
        setZoomLevel(transform.k);
        const moved = Math.abs(transform.x) > 1e-6 || Math.abs(transform.y) > 1e-6;
        setIsTransformed(transform.k !== 1 || moved);
      });
    // Keep the drawing in view: the world can't be panned off the map.
    if (fitWidth > 0 && fitHeight > 0) {
      zoomBehavior.translateExtent([
        [0, 0],
        [fitWidth, fitHeight],
      ]);
    }

    setZoomBehavior(() => zoomBehavior);
  }, [svgRef, fitWidth, fitHeight]);

  // Build the projection for the current type, fitted to the map's drawing size
  // when the caller gives one, and to the container on window resize otherwise
  useEffect(() => {
    const refit = (fit?: [number, number]) => {
      const newProjection = buildProjection(projectionType, fit);
      setProjection(() => newProjection);
      setPath(() => geoPath().projection(newProjection));
    };

    if (fitWidth > 0 && fitHeight > 0) {
      refit([fitWidth, fitHeight]);
      return;
    }

    const handleResize = () => {
      const rect = svgRef.current?.getBoundingClientRect();
      refit(rect ? [rect.width, rect.height] : undefined);
    };

    window.addEventListener('resize', handleResize);
    handleResize(); // Initial call

    return () => window.removeEventListener('resize', handleResize);
  }, [projectionType, svgRef, fitWidth, fitHeight]);

  // Zoom by a factor about the map's centre, within the zoom behavior's scale extent
  const zoomBy = useCallback(
    (factor: number) => {
      if (!svgRef.current || !zoomBehavior) return;
      d3.select(svgRef.current).call(zoomBehavior.scaleBy, factor);
    },
    [svgRef, zoomBehavior]
  );

  // Reset zoom and pan
  const resetZoom = useCallback(() => {
    if (!svgRef.current || !zoomBehavior) return;
    d3.select(svgRef.current).call(zoomBehavior.transform, d3.zoomIdentity);
  }, [svgRef, zoomBehavior]);

  return {
    projection,
    path,
    zoomBehavior,
    zoomLevel,
    isTransformed,
    zoomBy,
    resetZoom,
    projections: Object.keys(PROJECTIONS),
  };
};
