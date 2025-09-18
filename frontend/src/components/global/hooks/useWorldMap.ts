/**
 * useWorldMap Hook
 *
 * Custom hook for managing D3.js world map logic including projection,
 * path generation, zoom behavior, and responsive updates.
 */

import { useState, useEffect, useCallback } from 'react';
import * as d3 from 'd3';
import { geoPath, geoNaturalEarth1, geoMercator, geoOrthographic } from 'd3-geo';
import { zoom } from 'd3-zoom';

export interface MapProjection {
  name: string;
  projection: any;
  defaultScale: number;
  defaultCenter: [number, number];
}

const PROJECTIONS: Record<string, MapProjection> = {
  naturalEarth: {
    name: 'Natural Earth',
    projection: geoNaturalEarth1(),
    defaultScale: 150,
    defaultCenter: [0, 0],
  },
  mercator: {
    name: 'Mercator',
    projection: geoMercator(),
    defaultScale: 100,
    defaultCenter: [0, 0],
  },
  orthographic: {
    name: 'Orthographic',
    projection: geoOrthographic(),
    defaultScale: 200,
    defaultCenter: [0, 0],
  },
};

export const useWorldMap = (
  svgRef: React.RefObject<SVGSVGElement>,
  projectionType: string = 'naturalEarth'
) => {
  const [projection, setProjection] = useState(() => {
    const proj = PROJECTIONS[projectionType] || PROJECTIONS.naturalEarth;
    return proj.projection.scale(proj.defaultScale).center(proj.defaultCenter);
  });

  const [path, setPath] = useState(() => geoPath().projection(projection));

  const [zoomBehavior, setZoomBehavior] = useState<any>(null);

  // Update projection when type changes
  useEffect(() => {
    const proj = PROJECTIONS[projectionType] || PROJECTIONS.naturalEarth;
    const newProjection = proj.projection.scale(proj.defaultScale).center(proj.defaultCenter);

    setProjection(newProjection);
    setPath(geoPath().projection(newProjection));
  }, [projectionType]);

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

    setZoomBehavior(zoomBehavior);
  }, [svgRef]);

  // Handle window resize
  useEffect(() => {
    const handleResize = () => {
      if (!svgRef.current) return;

      const svg = d3.select(svgRef.current);
      const container = svg.node()?.parentElement;

      if (container) {
        const { width, height } = container.getBoundingClientRect();

        // Update projection to fit container
        const proj = PROJECTIONS[projectionType] || PROJECTIONS.naturalEarth;
        const newProjection = proj.projection.scale(Math.min(width, height) / 2).center([0, 0]);

        setProjection(newProjection);
        setPath(geoPath().projection(newProjection));
      }
    };

    window.addEventListener('resize', handleResize);
    handleResize(); // Initial call

    return () => window.removeEventListener('resize', handleResize);
  }, [projectionType, svgRef]);

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
