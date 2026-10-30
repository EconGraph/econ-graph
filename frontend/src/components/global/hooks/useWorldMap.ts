/**
 * UseWorldMap Hook.
 *
 * Custom hook for managing D3.js world map logic including projection,
 * path generation, zoom behavior, and responsive updates.
 */

/* global PointerEvent */

import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import * as d3 from 'd3';
import { geoPath, geoEqualEarth, geoNaturalEarth1, geoMercator, geoOrthographic } from 'd3-geo';
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

/** Equal-area default for comparing country and continent sizes. */
export const DEFAULT_PROJECTION = 'equalEarth';

export const PROJECTIONS: Record<string, MapProjection> = {
  equalEarth: {
    name: 'Equal Earth',
    create: geoEqualEarth,
    defaultScale: 150,
    defaultCenter: [0, 0],
  },
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
 * @param projectionType Key into PROJECTIONS; unknown types fall back to Equal Earth.
 * @param size Container width and height in pixels, when measured.
 * @returns A new projection.
 */
const buildProjection = (projectionType: string, size?: [number, number]): GeoProjection => {
  const proj = PROJECTIONS[projectionType] || PROJECTIONS[DEFAULT_PROJECTION];
  const projection = proj.create();
  if (size && size[0] > 0 && size[1] > 0) {
    return projection.fitSize(size, { type: 'Sphere' });
  }
  return projection.scale(proj.defaultScale).center(proj.defaultCenter);
};

export const useWorldMap = (
  svgRef: React.RefObject<SVGSVGElement | null>,
  projectionType = DEFAULT_PROJECTION,
  size?: [number, number]
) => {
  const [rotation, setRotation] = useState<[number, number]>([0, 0]);
  const rotationRef = useRef(rotation);
  const [fitWidth, fitHeight] = size ?? [0, 0];
  const [measuredSize, setMeasuredSize] = useState<[number, number]>();
  const projection = useMemo(() => {
    const next = buildProjection(
      projectionType,
      fitWidth > 0 && fitHeight > 0 ? [fitWidth, fitHeight] : measuredSize
    );
    return next.rotate([rotation[0], projectionType === 'orthographic' ? rotation[1] : 0]);
  }, [projectionType, fitWidth, fitHeight, measuredSize, rotation]);
  const path = useMemo(() => geoPath().projection(projection), [projection]);
  const projectionRef = useRef(projection);
  projectionRef.current = projection;
  const gestureActive = useRef(false);
  const suppressClick = useRef(false);
  const [zoomLevel, setZoomLevel] = useState(1);
  const [isPanned, setIsPanned] = useState(false);
  const zoomRef = useRef(1);

  const rotateBy = useCallback((longitude: number, latitude = 0) => {
    const previous = rotationRef.current;
    const next: [number, number] = [
      ((((previous[0] + longitude + 180) % 360) + 360) % 360) - 180,
      Math.max(-90, Math.min(90, previous[1] + latitude)),
    ];
    rotationRef.current = next;
    setRotation(next);
  }, []);

  // D3 handles wheel/button zoom. Single-pointer drags change the geography instead of
  // translating a fixed drawing; pointer pinches below use the same zoom transform.
  const zoomBehavior = useMemo(
    () =>
      zoom<SVGSVGElement, unknown>()
        .scaleExtent([MIN_ZOOM, MAX_ZOOM])
        .extent([
          [0, 0],
          [fitWidth || 960, fitHeight || 500],
        ])
        .translateExtent([
          [0, 0],
          [fitWidth || 960, fitHeight || 500],
        ])
        .filter(event => event.type === 'wheel')
        .on('zoom', event => {
          d3.select(svgRef.current).select('.map-container').attr('transform', event.transform);
          zoomRef.current = event.transform.k;
          setZoomLevel(event.transform.k);
          setIsPanned(Math.abs(event.transform.x) > 1e-6 || Math.abs(event.transform.y) > 1e-6);
        }),
    [svgRef, fitWidth, fitHeight]
  );

  useEffect(() => {
    if (fitWidth > 0 && fitHeight > 0) return;
    const resize = () => {
      const rect = svgRef.current?.getBoundingClientRect();
      if (rect && rect.width > 0 && rect.height > 0) setMeasuredSize([rect.width, rect.height]);
    };
    window.addEventListener('resize', resize);
    resize();
    return () => window.removeEventListener('resize', resize);
  }, [svgRef, fitWidth, fitHeight]);

  // A projection switch gets a fitted viewport while retaining geographic orientation.
  useEffect(() => {
    if (svgRef.current) d3.select(svgRef.current).call(zoomBehavior.transform, d3.zoomIdentity);
  }, [projectionType, zoomBehavior, svgRef]);

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const pointers = new Map<number, [number, number]>();
    let start: [number, number] | null = null;
    let dragged = false;
    let pinchDistance = 0;
    const distance = () => {
      const [a, b] = [...pointers.values()];
      return a && b ? Math.hypot(a[0] - b[0], a[1] - b[1]) : 0;
    };
    const down = (event: PointerEvent) => {
      if (event.button !== 0) return;
      if (pointers.size === 0) {
        start = [event.clientX, event.clientY];
        dragged = false;
        suppressClick.current = false;
      }
      pointers.set(event.pointerId, [event.clientX, event.clientY]);
      // Capture on the pressed shape first so ordinary clicks still target that country.
      // Once dragging starts, capture moves to the SVG for a stable interaction surface.
      (event.target as Element).setPointerCapture?.(event.pointerId);
      pinchDistance = distance();
      if (pointers.size > 1) {
        dragged = true;
        suppressClick.current = true;
        gestureActive.current = true;
      }
    };
    const move = (event: PointerEvent) => {
      const before = pointers.get(event.pointerId);
      if (!before) return;
      pointers.set(event.pointerId, [event.clientX, event.clientY]);
      if (!dragged && start && Math.hypot(event.clientX - start[0], event.clientY - start[1]) < 4)
        return;
      svg.setPointerCapture?.(event.pointerId);
      dragged = true;
      gestureActive.current = true;
      suppressClick.current = true;
      if (pointers.size > 1) {
        const nextDistance = distance();
        if (pinchDistance > 0 && nextDistance > 0) {
          const rect = svg.getBoundingClientRect();
          const [a, b] = [...pointers.values()];
          const point: [number, number] = [
            (((a[0] + b[0]) / 2 - rect.left) * (fitWidth || 960)) / (rect.width || 960),
            (((a[1] + b[1]) / 2 - rect.top) * (fitHeight || 500)) / (rect.height || 500),
          ];
          d3.select(svg).call(zoomBehavior.scaleBy, nextDistance / pinchDistance, point);
        }
        pinchDistance = nextDistance;
      } else {
        const rect = svg.getBoundingClientRect();
        const pixelsToDegrees = 180 / (Math.PI * projectionRef.current.scale() * zoomRef.current);
        const coordinateRatio = (fitWidth || 960) / (rect.width || fitWidth || 960);
        rotateBy(
          (event.clientX - before[0]) * coordinateRatio * pixelsToDegrees,
          projectionType === 'orthographic'
            ? -(event.clientY - before[1]) * coordinateRatio * pixelsToDegrees
            : 0
        );
        // At higher zoom, vertical drags pan the flat map while horizontal drags always
        // move around the world. Orthographic uses both axes to spin the globe.
        if (projectionType !== 'orthographic' && zoomRef.current > 1) {
          d3.select(svg).call(
            zoomBehavior.translateBy,
            0,
            ((event.clientY - before[1]) * coordinateRatio) / zoomRef.current
          );
        }
      }
      event.preventDefault();
    };
    const up = (event: PointerEvent) => {
      // A touch begins with implicit capture on its country path; transferring capture to
      // the SVG must not end that same gesture when the old path emits lostpointercapture.
      if (event.type === 'lostpointercapture' && event.target !== svg) return;
      pointers.delete(event.pointerId);
      pinchDistance = distance();
      if (pointers.size === 0) {
        gestureActive.current = false;
        start = null;
      }
      if (svg.hasPointerCapture?.(event.pointerId)) svg.releasePointerCapture(event.pointerId);
    };
    const click = (event: MouseEvent) => {
      if (suppressClick.current) {
        event.preventDefault();
        event.stopImmediatePropagation();
        suppressClick.current = false;
      }
    };
    svg.addEventListener('pointerdown', down);
    svg.addEventListener('pointermove', move);
    svg.addEventListener('pointerup', up);
    svg.addEventListener('pointercancel', up);
    svg.addEventListener('lostpointercapture', up);
    svg.addEventListener('click', click, true);
    return () => {
      svg.removeEventListener('pointerdown', down);
      svg.removeEventListener('pointermove', move);
      svg.removeEventListener('pointerup', up);
      svg.removeEventListener('pointercancel', up);
      svg.removeEventListener('lostpointercapture', up);
      svg.removeEventListener('click', click, true);
      gestureActive.current = false;
    };
  }, [svgRef, projectionType, rotateBy, zoomBehavior, fitWidth, fitHeight]);

  // Zoom by a factor about the map's centre, within the zoom behavior's scale extent
  const zoomBy = useCallback(
    (factor: number) => {
      if (!svgRef.current || !zoomBehavior) return;
      d3.select(svgRef.current).call(zoomBehavior.scaleBy, factor);
    },
    [svgRef, zoomBehavior]
  );

  // Reset the complete view, including geographic orientation.
  const resetZoom = useCallback(() => {
    rotationRef.current = [0, 0];
    setRotation([0, 0]);
    if (!svgRef.current) return;
    d3.select(svgRef.current).call(zoomBehavior.transform, d3.zoomIdentity);
  }, [svgRef, zoomBehavior]);

  return {
    projection,
    path,
    zoomBehavior,
    zoomLevel,
    isTransformed: zoomLevel !== 1 || isPanned || rotation.some(value => Math.abs(value) > 1e-6),
    rotateBy,
    gestureActive,
    zoomBy,
    resetZoom,
    projections: Object.keys(PROJECTIONS),
  };
};
