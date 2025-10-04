/**
 * Accessibility Utilities.
 *
 * Utility functions and constants for enhancing accessibility
 * in the Global Analysis components.
 */

import { CountryData } from '../../types/globalAnalysis';

/**
 * ARIA labels for map controls
 */
export const ARIA_LABELS = {
  MAP_CONTAINER: 'Interactive world map showing economic data by country',
  ZOOM_IN: 'Zoom in to see more detail',
  ZOOM_OUT: 'Zoom out to see more countries',
  RESET_ZOOM: 'Reset map to default view',
  PROJECTION_SELECTOR: 'Select map projection type',
  BORDERS_TOGGLE: 'Toggle country borders display',
  LABELS_TOGGLE: 'Toggle country name labels',
  COUNTRY_SELECTION: 'Selected countries for analysis',
  COLOR_LEGEND: 'Color scale legend showing data values',
  STATISTICAL_ANALYSIS: 'Statistical analysis of selected countries',
  EXPORT_BUTTON: 'Export selected data',
  COMPARE_BUTTON: 'Compare selected countries',
  CLEAR_SELECTION: 'Clear all selected countries',
} as const;

/**
 * Keyboard navigation keys
 */
export const KEYBOARD_KEYS = {
  ENTER: 'Enter',
  SPACE: ' ',
  ESCAPE: 'Escape',
  ARROW_UP: 'ArrowUp',
  ARROW_DOWN: 'ArrowDown',
  ARROW_LEFT: 'ArrowLeft',
  ARROW_RIGHT: 'ArrowRight',
  TAB: 'Tab',
  SHIFT_TAB: 'Shift+Tab',
} as const;

/**
 * Screen reader announcements
 */
export const SCREEN_READER_MESSAGES = {
  COUNTRY_SELECTED: (country: CountryData) =>
    `${country.name} selected. ${country.economicIndicators?.length || 0} economic indicators available.`,
  COUNTRY_DESELECTED: (country: CountryData) => `${country.name} deselected.`,
  ZOOM_CHANGED: (level: number) => `Map zoom level changed to ${Math.round(level * 100)}%.`,
  PROJECTION_CHANGED: (projection: string) => `Map projection changed to ${projection}.`,
  SELECTION_CLEARED: () => 'All countries deselected.',
  MAX_SELECTION_REACHED: (max: number) =>
    `Maximum ${max} countries selected. Deselect a country to select another.`,
  DATA_LOADED: (count: number) => `Map loaded with data for ${count} countries.`,
  NO_DATA_AVAILABLE: (country: CountryData) => `No economic data available for ${country.name}.`,
} as const;

/**
 * Focus management utilities
 */
export const focusManagement = {
  /**
   * Focus the first interactive element in a container
   */
  focusFirst: (container: HTMLElement) => {
    const focusableElements = container.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusableElements.length > 0) {
      (focusableElements[0] as HTMLElement).focus();
    }
  },

  /**
   * Focus the last interactive element in a container
   */
  focusLast: (container: HTMLElement) => {
    const focusableElements = container.querySelectorAll(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusableElements.length > 0) {
      (focusableElements[focusableElements.length - 1] as HTMLElement).focus();
    }
  },

  /**
   * Trap focus within a container
   */
  trapFocus: (container: HTMLElement, event: KeyboardEvent) => {
    if (event.key === 'Tab') {
      const focusableElements = container.querySelectorAll(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
      );
      const firstElement = focusableElements[0] as HTMLElement;
      const lastElement = focusableElements[focusableElements.length - 1] as HTMLElement;

      if (event.shiftKey) {
        if (document.activeElement === firstElement) {
          event.preventDefault();
          lastElement.focus();
        }
      } else {
        if (document.activeElement === lastElement) {
          event.preventDefault();
          firstElement.focus();
        }
      }
    }
  },
};

/**
 * Generate accessible country description
 */
export const getCountryDescription = (country: CountryData, selectedIndicator: string): string => {
  const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
  const value = indicator?.value;
  const unit = indicator?.unit || '';

  if (value !== undefined && value !== null) {
    const formattedValue = formatValueForScreenReader(value, unit);
    return `${country.name}, ${country.region}. ${selectedIndicator}: ${formattedValue}.`;
  }

  return `${country.name}, ${country.region}. No data available for ${selectedIndicator}.`;
};

/**
 * Format value for screen reader announcement
 */
export const formatValueForScreenReader = (value: number, unit: string): string => {
  if (value >= 1e12) {
    return `${(value / 1e12).toFixed(1)} trillion ${unit}`;
  } else if (value >= 1e9) {
    return `${(value / 1e9).toFixed(1)} billion ${unit}`;
  } else if (value >= 1e6) {
    return `${(value / 1e6).toFixed(1)} million ${unit}`;
  } else if (value >= 1e3) {
    return `${(value / 1e3).toFixed(1)} thousand ${unit}`;
  } else {
    return `${value.toFixed(1)} ${unit}`;
  }
};

/**
 * Generate accessible color legend description
 */
export const getColorLegendDescription = (
  indicator: string,
  minValue: number,
  maxValue: number,
  unit: string
): string => {
  const minFormatted = formatValueForScreenReader(minValue, unit);
  const maxFormatted = formatValueForScreenReader(maxValue, unit);

  return (
    `Color legend for ${indicator}. Values range from ${minFormatted} to ${maxFormatted}. ` +
    `Countries with higher values are shown in darker colors, ` +
    `countries with lower values are shown in lighter colors.`
  );
};

/**
 * Generate accessible statistical summary
 */
export const getStatisticalSummary = (countries: CountryData[], indicator: string): string => {
  const values = countries
    .map(country => {
      const indicatorData = country.economicIndicators?.find(ind => ind.name === indicator);
      return indicatorData?.value;
    })
    .filter(value => value !== undefined && value !== null) as number[];

  if (values.length === 0) {
    return `No data available for ${indicator} in selected countries.`;
  }

  const mean = values.reduce((sum, val) => sum + val, 0) / values.length;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const unit = countries[0]?.economicIndicators?.[0]?.unit || '';

  const meanFormatted = formatValueForScreenReader(mean, unit);
  const minFormatted = formatValueForScreenReader(min, unit);
  const maxFormatted = formatValueForScreenReader(max, unit);

  return (
    `Statistical analysis for ${indicator}: ${values.length} countries selected. ` +
    `Average value: ${meanFormatted}. ` +
    `Range: ${minFormatted} to ${maxFormatted}.`
  );
};

/**
 * Keyboard navigation handlers
 */
export const keyboardHandlers = {
  /**
   * Handle arrow key navigation for country selection
   */
  handleArrowKeys: (
    event: KeyboardEvent,
    countries: CountryData[],
    currentIndex: number,
    onCountrySelect: (country: CountryData) => void
  ) => {
    let newIndex = currentIndex;

    switch (event.key) {
      case KEYBOARD_KEYS.ARROW_UP:
      case KEYBOARD_KEYS.ARROW_LEFT:
        newIndex = Math.max(0, currentIndex - 1);
        break;
      case KEYBOARD_KEYS.ARROW_DOWN:
      case KEYBOARD_KEYS.ARROW_RIGHT:
        newIndex = Math.min(countries.length - 1, currentIndex + 1);
        break;
      default:
        return;
    }

    if (newIndex !== currentIndex) {
      event.preventDefault();
      onCountrySelect(countries[newIndex]);
    }
  },

  /**
   * Handle Enter/Space key for country selection
   */
  handleSelectionKeys: (
    event: KeyboardEvent,
    country: CountryData,
    onCountryClick: (country: CountryData) => void
  ) => {
    if (event.key === KEYBOARD_KEYS.ENTER || event.key === KEYBOARD_KEYS.SPACE) {
      event.preventDefault();
      onCountryClick(country);
    }
  },

  /**
   * Handle Enter/Space for generic activations (no item argument)
   */
  handleActivationKey: (event: KeyboardEvent, onActivate: () => void) => {
    if (event.key === KEYBOARD_KEYS.ENTER || event.key === KEYBOARD_KEYS.SPACE) {
      event.preventDefault();
      onActivate();
    }
  },

  /**
   * Handle Escape key for closing modals/panels
   */
  handleEscape: (event: KeyboardEvent, onClose: () => void) => {
    if (event.key === KEYBOARD_KEYS.ESCAPE) {
      event.preventDefault();
      onClose();
    }
  },
};

/**
 * High contrast mode detection
 */
export const isHighContrastMode = (): boolean => {
  if (typeof window === 'undefined') return false;

  // Check for Windows high contrast mode
  if (window.matchMedia('(-ms-high-contrast: active)').matches) {
    return true;
  }

  // Check for forced colors mode
  if (window.matchMedia('(forced-colors: active)').matches) {
    return true;
  }

  return false;
};

/**
 * Reduced motion detection
 */
export const prefersReducedMotion = (): boolean => {
  if (typeof window === 'undefined') return false;

  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
};

/**
 * Color contrast utilities
 */
export const colorContrast = {
  /**
   * Calculate relative luminance of a color
   */
  getLuminance: (r: number, g: number, b: number): number => {
    const [rs, gs, bs] = [r, g, b].map(c => {
      c = c / 255;
      return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs;
  },

  /**
   * Calculate contrast ratio between two colors
   */
  getContrastRatio: (
    color1: [number, number, number],
    color2: [number, number, number]
  ): number => {
    const lum1 = colorContrast.getLuminance(...color1);
    const lum2 = colorContrast.getLuminance(...color2);
    const brightest = Math.max(lum1, lum2);
    const darkest = Math.min(lum1, lum2);
    return (brightest + 0.05) / (darkest + 0.05);
  },

  /**
   * Check if contrast ratio meets WCAG AA standards
   */
  meetsWCAGAA: (color1: [number, number, number], color2: [number, number, number]): boolean => {
    return colorContrast.getContrastRatio(color1, color2) >= 4.5;
  },

  /**
   * Check if contrast ratio meets WCAG AAA standards
   */
  meetsWCAGAAA: (color1: [number, number, number], color2: [number, number, number]): boolean => {
    return colorContrast.getContrastRatio(color1, color2) >= 7;
  },
};
