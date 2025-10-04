/**
 * UseWorldMap Hook Tests.
 *
 * Test suite for the useWorldMap hook covering projection handling,
 * zoom behavior, and responsive updates.
 */

import { renderHook, act } from '@testing-library/react';
import { useWorldMap } from '../useWorldMap';
import { setupD3Mocks } from '../../../../test-utils/d3-testing-utils';

// Setup D3.js mocks
setupD3Mocks();

describe('useWorldMap', () => {
  let mockSvgRef: React.RefObject<SVGSVGElement>;

  beforeEach(() => {
    mockSvgRef = {
      current: document.createElementNS('http://www.w3.org/2000/svg', 'svg'),
    };
    vi.clearAllMocks();
  });

  describe('Initialization', () => {
    it('should initialize with default projection', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      expect(result.current.projection).toBeDefined();
      expect(result.current.path).toBeDefined();
      expect(result.current.zoomBehavior).toBeDefined();
    });

    it('should initialize with custom projection', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef, 'mercator'));

      expect(result.current.projection).toBeDefined();
      expect(result.current.path).toBeDefined();
    });

    it('should provide available projections', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      expect(result.current.projections).toContain('naturalEarth');
      expect(result.current.projections).toContain('mercator');
      expect(result.current.projections).toContain('orthographic');
    });
  });

  describe('Projection Changes', () => {
    it('should update projection when type changes', () => {
      const { result, rerender } = renderHook(
        ({ projectionType }) => useWorldMap(mockSvgRef, projectionType),
        { initialProps: { projectionType: 'naturalEarth' } }
      );


      rerender({ projectionType: 'mercator' });

      expect(result.current.projection).toBeDefined();
      // Projection should be different (though we can't easily test the actual D3 objects)
    });

    it('should handle invalid projection type', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef, 'invalid' as any));

      expect(result.current.projection).toBeDefined();
      expect(result.current.path).toBeDefined();
    });
  });

  describe('Zoom Behavior', () => {
    it('should create zoom behavior', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      expect(result.current.zoomBehavior).toBeDefined();
    });

    it('should provide zoom control functions', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      expect(typeof result.current.zoomToFit).toBe('function');
      expect(typeof result.current.zoomToCountry).toBe('function');
      expect(typeof result.current.resetZoom).toBe('function');
      expect(typeof result.current.getZoomLevel).toBe('function');
      expect(typeof result.current.getCenter).toBe('function');
    });

    it('should handle zoom to fit', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      act(() => {
        result.current.zoomToFit();
      });

      // Should not throw error
      expect(result.current.zoomToFit).toBeDefined();
    });

    it('should handle zoom to country', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      act(() => {
        result.current.zoomToCountry('US');
      });

      // Should not throw error
      expect(result.current.zoomToCountry).toBeDefined();
    });

    it('should handle reset zoom', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      act(() => {
        result.current.resetZoom();
      });

      // Should not throw error
      expect(result.current.resetZoom).toBeDefined();
    });

    it('should get current zoom level', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      const zoomLevel = result.current.getZoomLevel();
      expect(typeof zoomLevel).toBe('number');
    });

    it('should get current center', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      const center = result.current.getCenter();
      expect(Array.isArray(center)).toBe(true);
      expect(center).toHaveLength(2);
    });
  });

  describe('Responsive Updates', () => {
    it('should handle window resize', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef));

      // Simulate window resize
      act(() => {
        window.dispatchEvent(new Event('resize'));
      });

      expect(result.current.projection).toBeDefined();
    });

    it('should handle missing SVG ref', () => {
      const nullRef = { current: null };
      const { result } = renderHook(() => useWorldMap(nullRef));

      expect(result.current.projection).toBeDefined();
      expect(result.current.path).toBeDefined();
    });
  });

  describe('Edge Cases', () => {
    it('should handle undefined projection type', () => {
      const { result } = renderHook(() => useWorldMap(mockSvgRef, undefined as any));

      expect(result.current.projection).toBeDefined();
      expect(result.current.path).toBeDefined();
    });

    it('should handle null SVG ref', () => {
      const nullRef = { current: null };
      const { result } = renderHook(() => useWorldMap(nullRef));

      expect(result.current.projection).toBeDefined();
      expect(result.current.path).toBeDefined();
    });

    it('should handle SVG ref without parent element', () => {
      const svgElement = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      const ref = { current: svgElement };
      
      const { result } = renderHook(() => useWorldMap(ref));

      expect(result.current.projection).toBeDefined();
      expect(result.current.path).toBeDefined();
    });
  });

  describe('Memory Management', () => {
    it('should clean up event listeners on unmount', () => {
      const addEventListenerSpy = vi.spyOn(window, 'addEventListener');
      const removeEventListenerSpy = vi.spyOn(window, 'removeEventListener');

      const { unmount } = renderHook(() => useWorldMap(mockSvgRef));

      expect(addEventListenerSpy).toHaveBeenCalledWith('resize', expect.any(Function));

      unmount();

      expect(removeEventListenerSpy).toHaveBeenCalledWith('resize', expect.any(Function));
    });
  });
});
