/**
 * GlobalAnalysisContext
 *
 * React context for managing global analysis state including map view,
 * selected countries, indicators, and user preferences.
 */

import React, { createContext, useContext, useReducer, useCallback } from 'react';
import { CountryData, MapViewState, FilterState, UserPreferences } from '../types/globalAnalysis';

interface GlobalAnalysisState {
  // Map state
  mapView: MapViewState;
  selectedCountries: string[];
  hoveredCountry: string | null;

  // Data state
  countries: CountryData[];
  selectedIndicator: string;
  timeRange: { start: Date; end: Date };

  // UI state
  animationEnabled: boolean;
  showBorders: boolean;
  showLabels: boolean;
  labelSize: number;
  projection: string;
  colorScheme: string;

  // Filter state
  filters: FilterState;

  // User preferences
  preferences: UserPreferences;

  // Loading and error states
  loading: boolean;
  error: string | null;
}

interface GlobalAnalysisContextType {
  state: GlobalAnalysisState;
  actions: {
    setSelectedCountries: (countries: string[]) => void;
    toggleCountry: (countryId: string) => void;
    setHoveredCountry: (country: string | null) => void;
    setSelectedIndicator: (indicator: string) => void;
    setTimeRange: (range: { start: Date; end: Date }) => void;
    setMapView: (view: Partial<MapViewState>) => void;
    updateFilters: (filters: Partial<FilterState>) => void;
    setCountries: (countries: CountryData[]) => void;
    setProjection: (projection: string) => void;
    setColorScheme: (scheme: string) => void;
    toggleAnimation: () => void;
    toggleBorders: () => void;
    toggleLabels: () => void;
    setLabelSize: (size: number) => void;
    setLoading: (loading: boolean) => void;
    setError: (error: string | null) => void;
    updatePreferences: (preferences: Partial<UserPreferences>) => void;
    resetMap: () => void;
  };
}

// Initial state
const initialState: GlobalAnalysisState = {
  mapView: {
    scale: 150,
    translation: [480, 250],
  },
  selectedCountries: [],
  hoveredCountry: null,
  countries: [],
  selectedIndicator: 'GDP',
  timeRange: {
    start: new Date(2020, 0, 1),
    end: new Date(2023, 11, 31),
  },
  animationEnabled: true,
  showBorders: true,
  showLabels: false,
  labelSize: 12,
  projection: 'naturalEarth',
  colorScheme: 'viridis',
  filters: {
    selectedRegions: [],
    selectedSubregions: [],
    dateRange: {
      start: new Date(2020, 0, 1),
      end: new Date(2023, 11, 31),
    },
    selectedIndicators: [],
    completeDataOnly: false,
  },
  preferences: {
    theme: 'light',
    defaultChartType: 'line',
    notifications: true,
    collaborationEnabled: true,
    timezone: 'UTC',
    dateFormat: 'MM/DD/YYYY',
    numberFormat: 'en-US',
    language: 'en',
  },
  loading: false,
  error: null,
};

// Action types
type GlobalAnalysisAction =
  | { type: 'SET_SELECTED_COUNTRIES'; payload: string[] }
  | { type: 'TOGGLE_COUNTRY'; payload: string }
  | { type: 'SET_HOVERED_COUNTRY'; payload: string | null }
  | { type: 'SET_SELECTED_INDICATOR'; payload: string }
  | { type: 'SET_TIME_RANGE'; payload: { start: Date; end: Date } }
  | { type: 'SET_MAP_VIEW'; payload: Partial<MapViewState> }
  | { type: 'UPDATE_FILTERS'; payload: Partial<FilterState> }
  | { type: 'SET_COUNTRIES'; payload: CountryData[] }
  | { type: 'SET_PROJECTION'; payload: string }
  | { type: 'SET_COLOR_SCHEME'; payload: string }
  | { type: 'TOGGLE_ANIMATION' }
  | { type: 'TOGGLE_BORDERS' }
  | { type: 'TOGGLE_LABELS' }
  | { type: 'SET_LABEL_SIZE'; payload: number }
  | { type: 'SET_LOADING'; payload: boolean }
  | { type: 'SET_ERROR'; payload: string | null }
  | { type: 'UPDATE_PREFERENCES'; payload: Partial<UserPreferences> }
  | { type: 'RESET_MAP' };

// Reducer
const globalAnalysisReducer = (
  state: GlobalAnalysisState,
  action: GlobalAnalysisAction
): GlobalAnalysisState => {
  switch (action.type) {
    case 'SET_SELECTED_COUNTRIES':
      return {
        ...state,
        selectedCountries: action.payload,
      };

    case 'TOGGLE_COUNTRY':
      const countryId = action.payload;
      const isSelected = state.selectedCountries.includes(countryId);
      return {
        ...state,
        selectedCountries: isSelected
          ? state.selectedCountries.filter(id => id !== countryId)
          : [...state.selectedCountries, countryId],
      };

    case 'SET_HOVERED_COUNTRY':
      return {
        ...state,
        hoveredCountry: action.payload,
      };

    case 'SET_SELECTED_INDICATOR':
      return {
        ...state,
        selectedIndicator: action.payload,
      };

    case 'SET_TIME_RANGE':
      return {
        ...state,
        timeRange: action.payload,
        filters: {
          ...state.filters,
          dateRange: action.payload,
        },
      };

    case 'SET_MAP_VIEW':
      return {
        ...state,
        mapView: {
          ...state.mapView,
          ...action.payload,
        },
      };

    case 'UPDATE_FILTERS':
      return {
        ...state,
        filters: {
          ...state.filters,
          ...action.payload,
        },
      };

    case 'SET_COUNTRIES':
      return {
        ...state,
        countries: action.payload,
      };

    case 'SET_PROJECTION':
      return {
        ...state,
        projection: action.payload,
      };

    case 'SET_COLOR_SCHEME':
      return {
        ...state,
        colorScheme: action.payload,
      };

    case 'TOGGLE_ANIMATION':
      return {
        ...state,
        animationEnabled: !state.animationEnabled,
      };

    case 'TOGGLE_BORDERS':
      return {
        ...state,
        showBorders: !state.showBorders,
      };

    case 'TOGGLE_LABELS':
      return {
        ...state,
        showLabels: !state.showLabels,
      };

    case 'SET_LABEL_SIZE':
      return {
        ...state,
        labelSize: action.payload,
      };

    case 'SET_LOADING':
      return {
        ...state,
        loading: action.payload,
      };

    case 'SET_ERROR':
      return {
        ...state,
        error: action.payload,
      };

    case 'UPDATE_PREFERENCES':
      return {
        ...state,
        preferences: {
          ...state.preferences,
          ...action.payload,
        },
      };

    case 'RESET_MAP':
      return {
        ...state,
        mapView: {
          ...initialState.mapView,
        },
        selectedCountries: [],
        hoveredCountry: null,
      };

    default:
      return state;
  }
};

// Context
const GlobalAnalysisContext = createContext<GlobalAnalysisContextType | undefined>(undefined);

// Provider component
export const GlobalAnalysisProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [state, dispatch] = useReducer(globalAnalysisReducer, initialState);

  const actions = {
    setSelectedCountries: useCallback((countries: string[]) => {
      dispatch({ type: 'SET_SELECTED_COUNTRIES', payload: countries });
    }, []),

    toggleCountry: useCallback((countryId: string) => {
      dispatch({ type: 'TOGGLE_COUNTRY', payload: countryId });
    }, []),

    setHoveredCountry: useCallback((country: string | null) => {
      dispatch({ type: 'SET_HOVERED_COUNTRY', payload: country });
    }, []),

    setSelectedIndicator: useCallback((indicator: string) => {
      dispatch({ type: 'SET_SELECTED_INDICATOR', payload: indicator });
    }, []),

    setTimeRange: useCallback((range: { start: Date; end: Date }) => {
      dispatch({ type: 'SET_TIME_RANGE', payload: range });
    }, []),

    setMapView: useCallback((view: Partial<MapViewState>) => {
      dispatch({ type: 'SET_MAP_VIEW', payload: view });
    }, []),

    updateFilters: useCallback((filters: Partial<FilterState>) => {
      dispatch({ type: 'UPDATE_FILTERS', payload: filters });
    }, []),

    setCountries: useCallback((countries: CountryData[]) => {
      dispatch({ type: 'SET_COUNTRIES', payload: countries });
    }, []),

    setProjection: useCallback((projection: string) => {
      dispatch({ type: 'SET_PROJECTION', payload: projection });
    }, []),

    setColorScheme: useCallback((scheme: string) => {
      dispatch({ type: 'SET_COLOR_SCHEME', payload: scheme });
    }, []),

    toggleAnimation: useCallback(() => {
      dispatch({ type: 'TOGGLE_ANIMATION' });
    }, []),

    toggleBorders: useCallback(() => {
      dispatch({ type: 'TOGGLE_BORDERS' });
    }, []),

    toggleLabels: useCallback(() => {
      dispatch({ type: 'TOGGLE_LABELS' });
    }, []),

    setLabelSize: useCallback((size: number) => {
      dispatch({ type: 'SET_LABEL_SIZE', payload: size });
    }, []),

    setLoading: useCallback((loading: boolean) => {
      dispatch({ type: 'SET_LOADING', payload: loading });
    }, []),

    setError: useCallback((error: string | null) => {
      dispatch({ type: 'SET_ERROR', payload: error });
    }, []),

    updatePreferences: useCallback((preferences: Partial<UserPreferences>) => {
      dispatch({ type: 'UPDATE_PREFERENCES', payload: preferences });
    }, []),

    resetMap: useCallback(() => {
      dispatch({ type: 'RESET_MAP' });
    }, []),
  };

  return (
    <GlobalAnalysisContext.Provider value={{ state, actions }}>
      {children}
    </GlobalAnalysisContext.Provider>
  );
};

// Hook to use the context
export const useGlobalAnalysis = () => {
  const context = useContext(GlobalAnalysisContext);
  if (context === undefined) {
    throw new Error('useGlobalAnalysis must be used within a GlobalAnalysisProvider');
  }
  return context;
};

export default GlobalAnalysisContext;
