/**
 * Performance Utilities.
 *
 * Utility functions and hooks for optimizing performance in the
 * Global Analysis components.
 */

import React, { useCallback, useMemo, useRef, useEffect, useState } from 'react';
import { CountryData } from '../../types/globalAnalysis';
import * as d3 from 'd3';

/**
 * Debounce utility for performance optimization.
 * @param func - The function to debounce.
 * @param wait - The number of milliseconds to delay.
 * @returns The debounced function.
 */
export const debounce = <T extends (...args: any[]) => any>(
  func: T,
  wait: number
): ((...args: Parameters<T>) => void) => {
  let timeout: ReturnType<typeof setTimeout>;
  return (...args: Parameters<T>) => {
    clearTimeout(timeout);
    timeout = setTimeout(() => func(...args), wait);
  };
};

/**
 * Throttle utility for performance optimization.
 * @param func - The function to throttle.
 * @param limit - The number of milliseconds to throttle.
 * @returns The throttled function.
 */
export const throttle = <T extends (...args: any[]) => any>(
  func: T,
  limit: number
): ((...args: Parameters<T>) => void) => {
  let inThrottle: boolean;
  return (...args: Parameters<T>) => {
    if (!inThrottle) {
      func(...args);
      inThrottle = true;
      setTimeout(() => (inThrottle = false), limit);
    }
  };
};

/**
 * Memoized country data processing
 */
export const useMemoizedCountryData = (
  countries: CountryData[],
  selectedIndicator: string,
  colorScheme: string
) => {
  return useMemo(() => {
    const processedData = countries.map(country => {
      const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
      return {
        ...country,
        hasData: !!indicator,
        value: indicator?.value || 0,
        unit: indicator?.unit || '',
      };
    });

    const countriesWithData = processedData.filter(c => c.hasData);
    const countriesWithoutData = processedData.filter(c => !c.hasData);

    // Calculate data range
    const values = countriesWithData.map(c => c.value).filter(v => v > 0);
    const dataRange = {
      min: values.length > 0 ? Math.min(...values) : 0,
      max: values.length > 0 ? Math.max(...values) : 0,
    };

    return {
      processedData,
      countriesWithData,
      countriesWithoutData,
      dataRange,
    };
  }, [countries, selectedIndicator, colorScheme]);
};

/**
 * Memoized color scale generation
 */
export const useMemoizedColorScale = (
  dataRange: { min: number; max: number },
  colorScheme: string
) => {
  return useMemo(() => {
    if (dataRange.min === dataRange.max) {
      return (value: number) => '#d0d0d0';
    }

    // Create color scale based on scheme
    const getColorInterpolator = (scheme: string) => {
      switch (scheme) {
        case 'viridis':
          return (t: number) => {
            const colors = [
              [68, 1, 84],
              [72, 40, 120],
              [62, 73, 137],
              [49, 104, 142],
              [38, 130, 142],
              [31, 158, 137],
              [92, 200, 99],
              [170, 220, 50],
              [253, 231, 37],
            ];
            const i = Math.floor(t * (colors.length - 1));
            const j = Math.ceil(t * (colors.length - 1));
            const ratio = t * (colors.length - 1) - i;
            const color = colors[i].map((c, k) => Math.round(c + (colors[j][k] - c) * ratio));
            return `rgb(${color[0]}, ${color[1]}, ${color[2]})`;
          };
        case 'blues':
          return (t: number) => {
            const intensity = Math.floor(t * 255);
            return `rgb(${Math.floor(255 - intensity * 0.3)}, ${Math.floor(255 - intensity * 0.2)}, 255)`;
          };
        case 'reds':
          return (t: number) => {
            const intensity = Math.floor(t * 255);
            return `rgb(255, ${Math.floor(255 - intensity * 0.3)}, ${Math.floor(255 - intensity * 0.2)})`;
          };
        default:
          return (t: number) => `hsl(${t * 360}, 70%, 50%)`;
      }
    };

    const interpolator = getColorInterpolator(colorScheme);
    return (value: number) => {
      if (value === 0 || value === null || value === undefined) {
        return '#d0d0d0';
      }
      const normalized = (value - dataRange.min) / (dataRange.max - dataRange.min);
      return interpolator(Math.max(0, Math.min(1, normalized)));
    };
  }, [dataRange, colorScheme]);
};

/**
 * Memoized statistical calculations
 */
export const useMemoizedStatistics = (countries: CountryData[], selectedIndicator: string) => {
  return useMemo(() => {
    const values = countries
      .map(country => {
        const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
        return indicator?.value;
      })
      .filter(value => value !== undefined && value !== null) as number[];

    if (values.length === 0) {
      return {
        count: 0,
        mean: 0,
        median: 0,
        min: 0,
        max: 0,
        stdDev: 0,
        q25: 0,
        q75: 0,
        range: 0,
      };
    }

    const sortedValues = [...values].sort((a, b) => a - b);
    const count = values.length;
    const mean = values.reduce((sum, val) => sum + val, 0) / count;
    const median = sortedValues[Math.floor(count / 2)];
    const min = Math.min(...values);
    const max = Math.max(...values);
    const range = max - min;

    // Standard deviation
    const variance = values.reduce((sum, val) => sum + Math.pow(val - mean, 2), 0) / count;
    const stdDev = Math.sqrt(variance);

    // Quartiles
    const q25 = sortedValues[Math.floor(count * 0.25)];
    const q75 = sortedValues[Math.floor(count * 0.75)];

    return {
      count,
      mean,
      median,
      min,
      max,
      stdDev,
      q25,
      q75,
      range,
    };
  }, [countries, selectedIndicator]);
};

/**
 * Memoized region distribution
 */
export const useMemoizedRegionDistribution = (countries: CountryData[]) => {
  return useMemo(() => {
    const regions = countries.reduce(
      (acc, country) => {
        const key = country.region ?? 'Unknown';
        acc[key] = (acc[key] || 0) + 1;
        return acc;
      },
      {} as Record<string, number>
    );

    return Object.entries(regions).map(([region, count]) => ({
      region,
      count,
      percentage: (count / countries.length) * 100,
    }));
  }, [countries]);
};

/**
 * Memoized top/bottom countries
 */
export const useMemoizedTopBottomCountries = (
  countries: CountryData[],
  selectedIndicator: string,
  limit: number = 5
) => {
  return useMemo(() => {
    const countriesWithData = countries
      .map(country => {
        const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
        return {
          ...country,
          value: indicator?.value || 0,
        };
      })
      .filter(country => country.value > 0);

    const topCountries = [...countriesWithData].sort((a, b) => b.value - a.value).slice(0, limit);

    const bottomCountries = [...countriesWithData]
      .sort((a, b) => a.value - b.value)
      .slice(0, limit);

    return { topCountries, bottomCountries };
  }, [countries, selectedIndicator, limit]);
};

/**
 * Performance monitoring hook
 */
export const usePerformanceMonitor = (componentName: string) => {
  const renderCount = useRef(0);
  const startTime = useRef(Date.now());

  useEffect(() => {
    renderCount.current += 1;
    const renderTime = Date.now() - startTime.current;

    if (process.env.NODE_ENV === 'development') {
      // Performance monitoring in development mode
      // console.log(`${componentName} rendered ${renderCount.current} times in ${renderTime}ms`);
    }

    startTime.current = Date.now();
  });

  return {
    renderCount: renderCount.current,
    resetTimer: () => {
      startTime.current = Date.now();
      renderCount.current = 0;
    },
  };
};

/**
 * Memory management utilities
 */
export const memoryManagement = {
  /**
   * Clean up D3 selections and event listeners
   */
  cleanupD3Selection: (selection: any) => {
    if (selection && selection.remove) {
      selection.remove();
    }
  },

  /**
   * Clean up event listeners
   */
  cleanupEventListeners: (element: HTMLElement, events: string[]) => {
    events.forEach(event => {
      element.removeEventListener(event, () => {});
    });
  },

  /**
   * Clean up intervals and timeouts
   */
  cleanupTimers: (timers: (ReturnType<typeof setTimeout> | number)[]) => {
    timers.forEach(timer => {
      if (timer) {
        clearTimeout(timer);
        clearInterval(timer);
      }
    });
  },
};

/**
 * Virtual scrolling for large datasets
 */
export const useVirtualScrolling = (items: any[], itemHeight: number, containerHeight: number) => {
  const [scrollTop, setScrollTop] = useState(0);

  const visibleItems = useMemo(() => {
    const startIndex = Math.floor(scrollTop / itemHeight);
    const endIndex = Math.min(
      startIndex + Math.ceil(containerHeight / itemHeight) + 1,
      items.length
    );

    return items.slice(startIndex, endIndex).map((item, index) => ({
      ...item,
      index: startIndex + index,
    }));
  }, [items, scrollTop, itemHeight, containerHeight]);

  const totalHeight = items.length * itemHeight;
  const offsetY = scrollTop;

  return {
    visibleItems,
    totalHeight,
    offsetY,
    setScrollTop,
  };
};

/**
 * Intersection Observer for lazy loading
 */
export const useIntersectionObserver = (callback: (entries: any[]) => void, options: any = {}) => {
  const observerRef = useRef<any>(null);

  useEffect(() => {
    if (typeof window !== 'undefined' && 'IntersectionObserver' in window) {
      observerRef.current = new (window as any).IntersectionObserver(callback, options);
    }

    return () => {
      if (observerRef.current) {
        observerRef.current.disconnect();
      }
    };
  }, [callback, options]);

  return observerRef.current;
};

/**
 * Performance optimization for D3.js
 */
export const d3PerformanceOptimizations = {
  /**
   * Use D3's built-in data binding for efficient updates
   */
  efficientDataBinding: (selection: any, data: any[], key: (d: any) => string) => {
    const update = selection.data(data, key);
    const enter = update.enter();
    const exit = update.exit();

    return { update, enter, exit };
  },

  /**
   * Use D3's transition for smooth animations
   */
  smoothTransitions: (selection: any, duration: number = 300) => {
    return selection.transition().duration(duration);
  },

  /**
   * Use D3's brush for performance-optimized selection
   */
  optimizedBrush: (svg: any, width: number, height: number) => {
    return svg
      .append('g')
      .attr('class', 'brush')
      .call(
        d3
          .brush()
          .extent([
            [0, 0],
            [width, height],
          ])
          .on('brush', () => {})
      );
  },
};

/**
 * Bundle size optimization
 */
export const bundleOptimization = {
  /**
   * Lazy load heavy components
   */
  lazyLoadComponent: (importFn: () => Promise<any>) => {
    return React.lazy(importFn);
  },

  /**
   * Tree shake unused exports
   */
  treeShakeExports: (module: any, usedExports: string[]) => {
    const result: any = {};
    usedExports.forEach(exportName => {
      if (module[exportName]) {
        result[exportName] = module[exportName];
      }
    });
    return result;
  },
};
