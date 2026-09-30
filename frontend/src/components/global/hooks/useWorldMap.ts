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

const PROJECTIONS: Record<string, MapProjection> = {
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

  // Projections, path generators and zoom behaviors are all functions, so every
  // setter below wraps them in an updater; React would otherwise call them as one.

  // Create zoom behavior
  useEffect(() => {
    if (!svgRef.current) return;

    const zoomBehavior = zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.5, 8])
      .on('zoom', event => {
        const { transform } = event;
        const mapContainer = d3.select(svgRef.current).select('.map-container');
        mapContainer.attr('transform', transform);
      });

    setZoomBehavior(() => zoomBehavior);
  }, [svgRef]);

  const [fitWidth, fitHeight] = size ?? [0, 0];

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

  // Zoom to fit all countries
  const zoomToFit = useCallback(() => {
    if (!svgRef.current || !zoomBehavior) return;

    const svg = d3.select(svgRef.current);
    const node = svg.select('.countries').node() as any;
    const bounds = node?.getBBox?.() || { x: 0, y: 0, width: 0, height: 0 };

    if (bounds) {
      const { width, height } = svg.node()?.getBoundingClientRect() || { width: 800, height: 600 };
      const scale = Math.min(width / bounds.width, height / bounds.height) * 0.8;
      const translate = [
        width / 2 - scale * (bounds.x + bounds.width / 2),
        height / 2 - scale * (bounds.y + bounds.height / 2),
      ];

      svg
        .transition()
        .duration(750)
        .call(
          zoomBehavior.transform,
          d3.zoomIdentity.translate(translate[0], translate[1]).scale(scale)
        );
    }
  }, [svgRef, zoomBehavior]);

  // Zoom to specific country
  const zoomToCountry = useCallback(
    (countryCode: string) => {
      if (!svgRef.current || !zoomBehavior) return;

      const svg = d3.select(svgRef.current);
      const countryPath = svg.select(`path.country[data-country="${countryCode}"]`);

      if (countryPath.empty()) return;

      const node = countryPath.node() as any;
      const bounds = node?.getBBox?.() || { x: 0, y: 0, width: 0, height: 0 };
      if (!bounds) return;

      const { width, height } = svg.node()?.getBoundingClientRect() || { width: 800, height: 600 };
      const scale = Math.min(width / bounds.width, height / bounds.height) * 0.5;
      const translate = [
        width / 2 - scale * (bounds.x + bounds.width / 2),
        height / 2 - scale * (bounds.y + bounds.height / 2),
      ];

      svg
        .transition()
        .duration(750)
        .call(
          zoomBehavior.transform,
          d3.zoomIdentity.translate(translate[0], translate[1]).scale(scale)
        );
    },
    [svgRef, zoomBehavior]
  );

  // Reset zoom
  const resetZoom = useCallback(() => {
    if (!svgRef.current || !zoomBehavior) return;

    const svg = d3.select(svgRef.current);
    svg.transition().duration(750).call(zoomBehavior.transform, d3.zoomIdentity);
  }, [svgRef, zoomBehavior]);

  // Get current zoom level
  const getZoomLevel = useCallback(() => {
    if (!svgRef.current) return 1;

    const svg = d3.select(svgRef.current);
    const transform = d3.zoomTransform(svg.node() as Element);
    return transform.k;
  }, [svgRef]);

  // Get current center
  const getCenter = useCallback(() => {
    if (!svgRef.current) return [0, 0] as [number, number];

    const svg = d3.select(svgRef.current);
    const transform = d3.zoomTransform(svg.node() as Element);
    return [transform.x, transform.y] as [number, number];
  }, [svgRef]);

  return {
    projection,
    path,
    zoomBehavior,
    zoomToFit,
    zoomToCountry,
    resetZoom,
    getZoomLevel,
    getCenter,
    projections: Object.keys(PROJECTIONS),
  };
};
