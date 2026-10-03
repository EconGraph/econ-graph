/**
 * UseWorldMap Hook Tests.
 *
 * Ported from PR #154: projections, zoom and the resize listener.
 */

import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as d3 from 'd3';
import { useWorldMap } from '../useWorldMap';

// The shared setup file stubs d3-geo and d3-zoom; this hook needs the real ones.
vi.unmock('d3-geo');
vi.unmock('d3-zoom');

const svgRef = () => {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg') as SVGSVGElement;
  // d3-zoom takes the zoom extent from the viewBox, as on the map.
  svg.setAttribute('viewBox', '0 0 960 500');
  return { current: svg };
};

// Refs must outlive renders; a new one per render would re-run the hook's effects forever.
let REF: ReturnType<typeof svgRef>;
const NULL_REF = { current: null };

beforeEach(() => {
  REF = svgRef();
});

describe('useWorldMap', () => {
  describe('projections', () => {
    it('offers Natural Earth, Mercator and orthographic', () => {
      const { result } = renderHook(() => useWorldMap(REF));

      expect(result.current.projections).toEqual(['naturalEarth', 'mercator', 'orthographic']);
    });

    it('fits the projection to the given size', () => {
      const { result } = renderHook(() => useWorldMap(REF, 'naturalEarth', [960, 500]));

      // The globe's outline spans the full width.
      const [[x0], [x1]] = result.current.path.bounds({ type: 'Sphere' });
      expect(x0).toBeCloseTo(0, 0);
      expect(x1).toBeCloseTo(960, 0);
    });

    it('builds a new projection when the type changes', () => {
      const ref = svgRef();
      const { result, rerender } = renderHook(
        ({ type }) => useWorldMap(ref, type, [960, 500]),
        { initialProps: { type: 'naturalEarth' } }
      );
      const before = result.current.projection;

      rerender({ type: 'mercator' });

      expect(result.current.projection).not.toBe(before);
    });

    it('falls back to Natural Earth for an unknown type', () => {
      const { result: unknown } = renderHook(() => useWorldMap(REF, 'invalid', [960, 500]));
      const { result: natural } = renderHook(() => useWorldMap(REF, 'naturalEarth', [960, 500]));

      expect(unknown.current.projection([10, 20])).toEqual(natural.current.projection([10, 20]));
    });
  });

  describe('zoom', () => {
    it('zooms by a factor and resets', () => {
      const ref = svgRef();
      ref.current.appendChild(document.createElementNS('http://www.w3.org/2000/svg', 'g'))
        .setAttribute('class', 'map-container');
      const { result } = renderHook(() => useWorldMap(ref, 'naturalEarth', [960, 500]));
      expect(result.current.zoomLevel).toBe(1);

      act(() => result.current.zoomBy(2));
      expect(result.current.zoomLevel).toBe(2);
      expect(ref.current.querySelector('.map-container')?.getAttribute('transform')).toContain(
        'scale(2)'
      );

      act(() => result.current.resetZoom());
      expect(result.current.zoomLevel).toBe(1);
    });

    it('counts a pan as a change to reset, and keeps the world in view at full extent', () => {
      const { result } = renderHook(() => useWorldMap(REF, 'naturalEarth', [960, 500]));
      const svg = d3.select(REF.current);

      act(() => result.current.zoomBy(2));
      act(() => svg.call(result.current.zoomBehavior!.translateBy, 100, 0));
      expect(result.current.isTransformed).toBe(true);

      // Back at the whole world, the pan is clamped away: nothing left to reset.
      act(() => result.current.zoomBy(0.5));
      expect(result.current.zoomLevel).toBe(1);
      expect(d3.zoomTransform(REF.current)).toEqual(d3.zoomIdentity);
      expect(result.current.isTransformed).toBe(false);
    });

    it('keeps zoom between the whole world and eight times', () => {
      const { result } = renderHook(() => useWorldMap(REF, 'naturalEarth', [960, 500]));

      act(() => result.current.zoomBy(0.5));
      expect(result.current.zoomLevel).toBe(1);

      act(() => result.current.zoomBy(100));
      expect(result.current.zoomLevel).toBe(8);
    });

    it('does nothing without an SVG', () => {
      const { result } = renderHook(() => useWorldMap(NULL_REF));

      act(() => result.current.zoomBy(2));
      act(() => result.current.resetZoom());
      expect(result.current.zoomLevel).toBe(1);
    });
  });

  describe('resize', () => {
    it('listens for resizes only without a given size, and stops on unmount', () => {
      const add = vi.spyOn(window, 'addEventListener');
      const remove = vi.spyOn(window, 'removeEventListener');

      renderHook(() => useWorldMap(REF, 'naturalEarth', [960, 500])).unmount();
      expect(add).not.toHaveBeenCalledWith('resize', expect.any(Function));

      const { unmount } = renderHook(() => useWorldMap(REF));
      expect(add).toHaveBeenCalledWith('resize', expect.any(Function));
      unmount();
      expect(remove).toHaveBeenCalledWith('resize', expect.any(Function));

      add.mockRestore();
      remove.mockRestore();
    });
  });
});
