/**
 * Type definitions for Global Analysis features
 *
 * This file contains all TypeScript interfaces and types used throughout
 * the global analysis components, including world map, country data,
 * economic indicators, and user interactions.
 */

export interface CountryData {
  /** Unique identifier for the country */
  id: string;
  /** Display name of the country */
  name: string;
  /** ISO 3166-1 alpha-2 country code (e.g., 'US', 'CA') */
  isoAlpha2: string;
  /** ISO 3166-1 alpha-3 country code (e.g., 'USA', 'CAN') */
  isoAlpha3: string;
  /** Latitude coordinate for map positioning */
  latitude: number;
  /** Longitude coordinate for map positioning */
  longitude: number;
  /** GDP in USD (optional) */
  gdpUsd?: number;
  /** Population count (optional) */
  population?: number;
  /** Geographic region (e.g., 'North America', 'Europe') */
  region?: string;
  /** Geographic subregion (e.g., 'Northern America', 'Western Europe') */
  subregion?: string;
  /** Array of economic indicators for this country */
  economicIndicators?: EconomicIndicatorData[];
  /** Last updated timestamp */
  lastUpdated?: string;
  /** Data source information */
  dataSource?: string;
}

export interface EconomicIndicatorData {
  /** Name of the economic indicator */
  name: string;
  /** Current value of the indicator */
  value: number;
  /** Unit of measurement (e.g., 'USD', '%', 'thousands') */
  unit: string;
  /** Year of the data point */
  year: number;
  /** Source of the data */
  source: string;
  /** Description of what this indicator measures */
  description?: string;
  /** Category of the indicator (e.g., 'GDP', 'Inflation', 'Employment') */
  category?: string;
  /** Frequency of data updates (e.g., 'Annual', 'Quarterly', 'Monthly') */
  frequency?: string;
}

export interface MapViewState {
  /** Scale factor for zoom */
  scale: number;
  /** Translation offset [x, y] */
  translation: [number, number];
  /** Rotation for orthographic projection [x, y, z] */
  rotation?: [number, number, number];
}

export interface FilterState {
  /** Selected regions for filtering */
  selectedRegions: string[];
  /** Selected subregions for filtering */
  selectedSubregions: string[];
  /** Date range for data filtering */
  dateRange: {
    start: Date;
    end: Date;
  };
  /** Selected economic indicators */
  selectedIndicators: string[];
  /** Minimum GDP threshold */
  minGdp?: number;
  /** Maximum GDP threshold */
  maxGdp?: number;
  /** Minimum population threshold */
  minPopulation?: number;
  /** Maximum population threshold */
  maxPopulation?: number;
  /** Whether to show only countries with complete data */
  completeDataOnly: boolean;
}

export interface CustomGrouping {
  /** Unique identifier for the grouping */
  id: string;
  /** Display name of the grouping */
  name: string;
  /** Array of country IDs in this grouping */
  countryIds: string[];
  /** Color for this grouping on the map */
  color: string;
  /** Description of the grouping */
  description?: string;
  /** Whether this is a user-created grouping */
  isUserCreated: boolean;
}

export interface GlobalEvent {
  /** Unique identifier for the event */
  id: string;
  /** Title of the event */
  title: string;
  /** Description of the event */
  description: string;
  /** Date when the event occurred */
  date: Date;
  /** Type of event (e.g., 'Economic', 'Political', 'Natural Disaster') */
  type: string;
  /** Severity level (1-5, where 5 is most severe) */
  severity: number;
  /** Countries affected by this event */
  affectedCountries: string[];
  /** Source of the event information */
  source: string;
  /** URL for more information */
  url?: string;
  /** Tags for categorization */
  tags: string[];
}

export interface CountryImpact {
  /** Country ID */
  countryId: string;
  /** Impact severity (1-5) */
  impactSeverity: number;
  /** Economic impact description */
  economicImpact: string;
  /** Recovery time estimate in months */
  recoveryTimeMonths?: number;
  /** Confidence level in impact assessment (0-1) */
  confidence: number;
}

export interface EventPropagationData {
  /** Source country ID */
  sourceCountry: string;
  /** Target country ID */
  targetCountry: string;
  /** Propagation strength (0-1) */
  strength: number;
  /** Time delay in days */
  timeDelayDays: number;
  /** Type of propagation (e.g., 'Trade', 'Financial', 'Migration') */
  type: string;
}

export interface CorrelationData {
  /** First country ID */
  country1: string;
  /** Second country ID */
  country2: string;
  /** Correlation coefficient (-1 to 1) */
  correlation: number;
  /** Statistical significance (p-value) */
  pValue: number;
  /** Time period for correlation calculation */
  timePeriod: {
    start: Date;
    end: Date;
  };
  /** Indicator used for correlation */
  indicator: string;
}

export interface TradeFlowData {
  /** Source country ID */
  sourceCountry: string;
  /** Target country ID */
  targetCountry: string;
  /** Trade value in USD */
  value: number;
  /** Trade volume in units */
  volume?: number;
  /** Type of trade (e.g., 'Goods', 'Services', 'Capital') */
  type: string;
  /** Year of the trade data */
  year: number;
  /** Direction of trade flow */
  direction: 'import' | 'export' | 'both';
}

export interface MapProjection {
  /** Name of the projection */
  name: string;
  /** Display label for the projection */
  label: string;
  /** D3 projection function */
  projection: any;
  /** Default scale for this projection */
  defaultScale: number;
  /** Default center for this projection */
  defaultCenter: [number, number];
  /** Description of the projection */
  description: string;
}

export interface ColorSchemeData {
  /** Name of the color scheme */
  name: string;
  /** Display label for the color scheme */
  label: string;
  /** D3 color scale function */
  scale: any;
  /** Whether this scheme is colorblind-friendly */
  colorblindFriendly: boolean;
  /** Description of the color scheme */
  description: string;
}

export interface ExportSettings {
  /** Export format */
  format: 'PNG' | 'SVG' | 'PDF' | 'CSV' | 'JSON';
  /** Image resolution (for PNG/PDF) */
  resolution: number;
  /** Image width in pixels */
  width: number;
  /** Image height in pixels */
  height: number;
  /** Whether to include legend */
  includeLegend: boolean;
  /** Whether to include title */
  includeTitle: boolean;
  /** Custom title for export */
  customTitle?: string;
  /** Whether to include data table */
  includeData: boolean;
}

export interface ShareOptions {
  /** Title for the shared content */
  title: string;
  /** Description of the shared content */
  description: string;
  /** Whether to include data in the share */
  includeData: boolean;
  /** Whether to include annotations */
  includeAnnotations: boolean;
  /** Whether the share is public */
  public: boolean;
  /** Expiration date for the share */
  expiresAt?: Date;
  /** Password protection for the share */
  password?: string;
}

export interface NotificationSettings {
  /** Whether real-time updates are enabled */
  realTimeUpdates: boolean;
  /** Update frequency in milliseconds */
  updateFrequency: number;
  /** Whether to show push notifications */
  pushNotifications: boolean;
  /** Whether to show email notifications */
  emailNotifications: boolean;
  /** Notification types to receive */
  notificationTypes: string[];
}

export interface UserPreferences {
  /** Theme preference */
  theme: 'light' | 'dark' | 'auto';
  /** Default chart type */
  defaultChartType: 'line' | 'bar' | 'area' | 'scatter';
  /** Whether notifications are enabled */
  notifications: boolean;
  /** Whether collaboration is enabled */
  collaborationEnabled: boolean;
  /** User's timezone */
  timezone: string;
  /** Date format preference */
  dateFormat: string;
  /** Number format preference */
  numberFormat: string;
  /** Language preference */
  language: string;
}

export interface AnalysisResult {
  /** Type of analysis performed */
  analysisType: 'correlation' | 'regression' | 'forecasting' | 'trend';
  /** Results of the analysis */
  results: any;
  /** Confidence level in the results (0-1) */
  confidence: number;
  /** Statistical significance (p-value) */
  significance: number;
  /** Date when analysis was performed */
  performedAt: Date;
  /** Parameters used for the analysis */
  parameters: Record<string, any>;
  /** Recommendations based on the analysis */
  recommendations?: string[];
}

export interface ChartConfig {
  /** Chart type */
  type: 'line' | 'bar' | 'area' | 'scatter' | 'pie' | 'donut';
  /** Chart title */
  title: string;
  /** X-axis configuration */
  xAxis: {
    label: string;
    type: 'linear' | 'time' | 'category';
    min?: number;
    max?: number;
  };
  /** Y-axis configuration */
  yAxis: {
    label: string;
    type: 'linear' | 'log' | 'time';
    min?: number;
    max?: number;
  };
  /** Color scheme for the chart */
  colorScheme: string;
  /** Whether to show legend */
  showLegend: boolean;
  /** Whether to show grid lines */
  showGrid: boolean;
  /** Whether to show data labels */
  showDataLabels: boolean;
  /** Animation configuration */
  animation: {
    enabled: boolean;
    duration: number;
    easing: string;
  };
}

export interface DataPoint {
  /** X-axis value */
  x: number | string | Date;
  /** Y-axis value */
  y: number;
  /** Additional data for this point */
  metadata?: Record<string, any>;
  /** Color for this point */
  color?: string;
  /** Label for this point */
  label?: string;
}

export interface SeriesData {
  /** Unique identifier for the series */
  id: string;
  /** Display name for the series */
  name: string;
  /** Data points for the series */
  data: DataPoint[];
  /** Color for the series */
  color: string;
  /** Y-axis for this series */
  yAxis: 'left' | 'right';
  /** Whether the series is visible */
  visible: boolean;
  /** Chart type for this series */
  chartType: 'line' | 'bar' | 'area' | 'scatter';
}

export interface DateRange {
  /** Start date */
  start: Date;
  /** End date */
  end: Date;
}

export interface Viewport {
  /** Width of the viewport */
  width: number;
  /** Height of the viewport */
  height: number;
  /** Whether this is a mobile viewport */
  isMobile: boolean;
  /** Whether this is a tablet viewport */
  isTablet: boolean;
  /** Whether this is a desktop viewport */
  isDesktop: boolean;
}

export interface LoadingState {
  /** Whether data is currently loading */
  isLoading: boolean;
  /** Loading message to display */
  message?: string;
  /** Progress percentage (0-100) */
  progress?: number;
  /** Whether there was an error loading */
  hasError: boolean;
  /** Error message if loading failed */
  errorMessage?: string;
}

export interface PerformanceMetrics {
  /** Time taken to render the map (ms) */
  renderTime: number;
  /** Time taken to update data (ms) */
  updateTime: number;
  /** Memory usage in MB */
  memoryUsage: number;
  /** Number of countries rendered */
  countryCount: number;
  /** Frame rate during interactions */
  frameRate: number;
}

// Type aliases for better type safety
export type EconomicIndicator = 'gdp' | 'inflation' | 'unemployment';
export type ProjectionType = 'geoNaturalEarth1' | 'geoMercator' | 'geoOrthographic';
export type ColorScheme = 'viridis' | 'blues' | 'reds';

// Global Analysis State and Actions
export interface GlobalAnalysisState {
  selectedCountries: string[];
  selectedIndicator: EconomicIndicator;
  mapViewState: MapViewState;
  projectionType: ProjectionType;
  colorScheme: ColorScheme;
  hoveredCountry: string | null;
}

export interface GlobalAnalysisActions {
  selectCountry: (countryId: string) => void;
  deselectCountry: (countryId: string) => void;
  toggleCountry: (countryId: string) => void;
  setSelectedIndicator: (indicator: EconomicIndicator) => void;
  setMapViewState: (viewState: MapViewState) => void;
  setProjectionType: (type: ProjectionType) => void;
  setColorScheme: (scheme: ColorScheme) => void;
  setHoveredCountry: (countryId: string | null) => void;
}

export interface GlobalAnalysisContextType extends GlobalAnalysisState, GlobalAnalysisActions {}
