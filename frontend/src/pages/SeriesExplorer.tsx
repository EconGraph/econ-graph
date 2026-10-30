import React from 'react';
import {
  Box,
  Typography,
  TextField,
  Grid,
  Card,
  CardContent,
  CardActions,
  Button,
  Chip,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Pagination,
  Paper,
  IconButton,
  Skeleton,
  Collapse,
  Divider,
  Menu,
  Alert,
  CircularProgress,
  Snackbar,
} from '@mui/material';
import {
  Search as SearchIcon,
  TrendingUp as TrendingUpIcon,
  AccessTime as AccessTimeIcon,
  FileDownload as ExportIcon,
  Tune as AdvancedIcon,
  Clear as ClearIcon,
} from '@mui/icons-material';
import { useLocation, useNavigate, useNavigationType, useSearchParams } from 'react-router-dom';
import { useSeriesSearch, useSeriesBrowse, useDataSources } from '../hooks/useSeriesData';
import { buildSearchResultsCsv, buildSearchResultsJson } from '../utils/searchResultsExport';
import { downloadCsv } from '../utils/seriesCsv';

interface EconomicSeries {
  id: string;
  /** The source's own series id, such as `GDP`. */
  externalId?: string;
  title: string;
  description?: string;
  sourceId: string;
  /** Source name; missing when neither the result nor the source list names it. */
  source?: string;
  frequency: string;
  units?: string;
  lastUpdated?: string;
  startDate?: string;
  endDate?: string;
}

/** Most results the page asks the backend for in one search. */
const SEARCH_LIMIT = 100;

/** How long typing must pause before the search is sent. */
const SEARCH_DEBOUNCE_MS = 250;

/** Results per page. */
const PAGE_SIZE = 20;

/** Highest page a `page` URL parameter can ask for. A page past the last shows the last. */
const MAX_PAGE = 100_000;

/** The result orders, as the `sort` URL parameter names them. */
const SORT_ORDERS = ['relevance', 'title', 'lastUpdated'] as const;
type SortOrder = (typeof SORT_ORDERS)[number];

/**
 * Reads the `page` URL parameter: a positive whole number, or page 1 for anything else.
 * @param value - The raw `page` URL parameter, if any.
 * @returns The 1-based page.
 */
export const parsePageParam = (value: string | null): number => {
  if (!value || !/^\d+$/.test(value)) return 1;
  return Math.min(Math.max(Number(value), 1), MAX_PAGE);
};

/**
 * Reads the `sort` URL parameter: one of the result orders, or relevance for anything else.
 * @param value - The raw `sort` URL parameter, if any.
 * @returns The sort order.
 */
export const parseSortParam = (value: string | null): SortOrder =>
  SORT_ORDERS.find(order => order === value) ?? 'relevance';

export interface DataSourceOption {
  id: string;
  name: string;
}

/**
 * Formats an ISO date or timestamp as YYYY-MM-DD, or returns undefined when the backend
 * didn't send one. Missing dates are shown as missing, never replaced with made-up values.
 * @param value - ISO date or timestamp from the API, if any.
 * @returns The date part, or undefined.
 */
const toDateOnly = (value?: string | null): string | undefined => toTimestamp(value)?.split('T')[0];

/**
 * Normalizes an ISO date or timestamp to a full ISO timestamp, or undefined when missing.
 * @param value - ISO date or timestamp from the API, if any.
 * @returns The ISO timestamp, or undefined.
 */
const toTimestamp = (value?: string | null): string | undefined => {
  if (!value) return undefined;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? undefined : date.toISOString();
};

/** The backend's SeriesFrequency GraphQL enum, keyed by the frequency string series carry. */
const BACKEND_FREQUENCIES = new Set([
  'Daily',
  'Weekly',
  'Monthly',
  'Quarterly',
  'Annual',
  'Irregular',
]);

/**
 * Resolves a `source` URL parameter to a listed source's id. The parameter is normally
 * already an id, but older links and bookmarks put the source's
 * display name in the URL instead, and that name doesn't always match exactly (it may
 * be missing a "(FRED)"-style suffix) — so a name match falls back to a prefix match.
 * @param param - The raw `source` URL parameter, if any.
 * @param sources - The sources the backend lists.
 * @returns The matching source's id, or undefined when nothing matches.
 */
export const resolveSourceId = (param: string, sources: DataSourceOption[]): string | undefined => {
  if (!param) return undefined;
  const byId = sources.find(source => source.id === param);
  if (byId) return byId.id;
  const lowerParam = param.toLowerCase();
  const byName = sources.find(source => source.name.toLowerCase() === lowerParam);
  if (byName) return byName.id;
  const prefixMatches = sources.filter(source => source.name.toLowerCase().startsWith(lowerParam));
  return prefixMatches.length === 1 ? prefixMatches[0].id : undefined;
};

/**
 * The frequency as the backend's SeriesFrequency enum names it (its variants, upper-cased),
 * or undefined when it isn't one of the backend's known frequencies. An unrecognized value
 * (crawler data doesn't guarantee it matches) is left to the page's own client-side filter
 * instead of being sent as an invalid enum value.
 * @param frequency - The frequency as shown on a result.
 * @returns The GraphQL enum value, or undefined.
 */
const toBackendFrequency = (frequency: string): string | undefined =>
  BACKEND_FREQUENCIES.has(frequency) ? frequency.toUpperCase() : undefined;

/**
 * What the result count says while browsing a source without a query.
 * @param shown - Series left after the page's own frequency filter.
 * @param loaded - Series the backend returned for this page, by title.
 * @param offset - Series in title order before this page.
 * @param total - Series the source has under the backend's filters.
 * @returns The message.
 */
const browseMessage = (shown: number, loaded: number, offset: number, total: number): string => {
  const n = (x: number) => x.toLocaleString();
  if (total <= loaded) {
    return shown < loaded
      ? `${n(shown)} matching series of ${n(total)}, by title`
      : `${n(total)} series, by title`;
  }
  const range = `${n(offset + 1)}–${n(offset + loaded)} of ${n(total)} series, by title`;
  return shown < loaded ? `${n(shown)} matching series among ${range}` : range;
};

/**
 * REQUIREMENT: Browse and search functionality similar to FRED but more modern
 * PURPOSE: Provide comprehensive search and filtering for economic time series
 * This improves on FRED's search with better filters and modern UI patterns.
 * @returns JSX element representing the SeriesExplorer page.
 */
const SeriesExplorer: React.FC = () => {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  // State management
  const [searchQuery, setSearchQuery] = React.useState(searchParams.get('q') || '');
  const [selectedSource, setSelectedSource] = React.useState(searchParams.get('source') || '');
  const [selectedFrequency, setSelectedFrequency] = React.useState(
    searchParams.get('frequency') || ''
  );
  const [currentPage, setCurrentPage] = React.useState(() =>
    parsePageParam(searchParams.get('page'))
  );
  const [sortBy, setSortBy] = React.useState<SortOrder>(() =>
    parseSortParam(searchParams.get('sort'))
  );

  // Send the search once typing pauses, not on every keystroke
  const [debouncedQuery, setDebouncedQuery] = React.useState(searchQuery.trim());

  // The form follows the URL when something else changes it while the page stays mounted: a
  // link to another /explore URL (the header search), back and forward. The URL writes
  // the form makes below are marked in their history state and skipped here, so a write that
  // lands after more typing doesn't put the older text back in the box.
  const location = useLocation();
  const navigationType = useNavigationType();
  const [syncedLocationKey, setSyncedLocationKey] = React.useState(location.key);
  if (location.key !== syncedLocationKey) {
    setSyncedLocationKey(location.key);
    const formWrite =
      navigationType === 'REPLACE' &&
      (location.state as { explorerForm?: boolean } | null)?.explorerForm === true;
    if (!formWrite) {
      const query = searchParams.get('q') || '';
      setSearchQuery(query);
      setDebouncedQuery(query.trim());
      setSelectedSource(searchParams.get('source') || '');
      setSelectedFrequency(searchParams.get('frequency') || '');
      setCurrentPage(parsePageParam(searchParams.get('page')));
      setSortBy(parseSortParam(searchParams.get('sort')));
    }
  }

  // Advanced search state
  const [showAdvancedSearch, setShowAdvancedSearch] = React.useState(false);

  // Export and UI state
  const [exportMenuAnchor, setExportMenuAnchor] = React.useState<null | HTMLElement>(null);
  const [snackbarOpen, setSnackbarOpen] = React.useState(false);
  const [snackbarMessage, setSnackbarMessage] = React.useState('');

  // Search input ref for keyboard shortcuts
  const searchInputRef = React.useRef<HTMLInputElement>(null);

  // Data sources for the source filter, as the backend lists them
  const dataSourcesResult = useDataSources();
  const dataSources: DataSourceOption[] = React.useMemo(
    () => dataSourcesResult?.data ?? [],
    [dataSourcesResult?.data]
  );

  // Only a source the backend listed is used, so an old link that put the source name in
  // the URL resolves to that source's id instead of matching nothing. While the list is
  // loading, the source from the URL is held and the search waits for it.
  const resolvedSourceId = resolveSourceId(selectedSource, dataSources);
  const sourceListed = resolvedSourceId !== undefined;
  const waitingForSources = Boolean(selectedSource) && Boolean(dataSourcesResult?.isPending);
  const activeSource = sourceListed ? resolvedSourceId : waitingForSources ? selectedSource : '';

  React.useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(searchQuery.trim()), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  const backendFrequency = selectedFrequency ? toBackendFrequency(selectedFrequency) : undefined;

  // Search runs on the backend; the source filter is applied there too
  const searchResult = useSeriesSearch(
    debouncedQuery,
    {
      sourceId: activeSource || undefined,
      frequency: backendFrequency,
      limit: SEARCH_LIMIT,
    },
    !waitingForSources
  );
  // With no query but a source picked (the data sources page's Browse series), list what
  // that source has instead of an empty page. Only a source the backend
  // listed counts, so an unknown name in the URL still asks for a search; a frequency alone
  // isn't browsed, as it would sort the whole catalog. The backend pages through the source
  // in title order, so every series in it can be reached. A frequency the backend doesn't know
  // is filtered here instead, which a page at a time would leave mostly empty pages, so then
  // the first SEARCH_LIMIT series are listed and paged here, as a search's are.
  const browsing = debouncedQuery.length < 2 && Boolean(activeSource) && !waitingForSources;
  const pagedByBackend = browsing && (!selectedFrequency || Boolean(backendFrequency));
  const browseOffset = pagedByBackend ? (currentPage - 1) * PAGE_SIZE : 0;
  const browseResult = useSeriesBrowse(
    {
      sourceId: activeSource || undefined,
      frequency: backendFrequency,
      limit: pagedByBackend ? PAGE_SIZE : SEARCH_LIMIT,
      offset: browseOffset,
    },
    browsing
  );
  const activeResult = browsing ? browseResult : searchResult;
  const searchResults = browsing ? browseResult?.data?.series : searchResult?.data;
  const browseTotal = browseResult?.data?.totalCount ?? 0;
  const { isLoading: isSearchLoading, error: searchError } = activeResult || {};
  // The Search button reflects the search request only, not a browse in the background
  const isSearchFetching = !browsing && Boolean(searchResult?.isFetching);
  const hasQuery = debouncedQuery.length >= 2 || browsing;
  const showLoading =
    isSearchLoading ||
    (waitingForSources && (debouncedQuery.length >= 2 || Boolean(selectedSource)));

  // Transform search results to the card format. Missing fields stay missing.
  const allSeries: EconomicSeries[] = React.useMemo(() => {
    if (!searchResults) return [];

    return searchResults.map((result: any) => ({
      id: result.id,
      externalId: result.externalId || undefined,
      title: result.title,
      description: result.description || undefined,
      sourceId: result.sourceId,
      source:
        result.source?.name || dataSources.find(ds => ds.id === result.sourceId)?.name || undefined,
      frequency: result.frequency,
      units: result.units || undefined,
      lastUpdated: toTimestamp(result.lastUpdated),
      startDate: toDateOnly(result.startDate),
      endDate: toDateOnly(result.endDate),
    }));
  }, [searchResults, dataSources]);

  // Apply the frequency filter (the backend filters by source), and sort
  const filteredSeries = React.useMemo(() => {
    let filtered = allSeries;

    // A frequency the backend knows was already applied there by prefix ("Weekly" matches
    // "Weekly, Ending Friday"), so match the same way; any other value is matched exactly here.
    if (selectedFrequency) {
      // The backend's prefixes for each frequency (SeriesFrequency::raw_prefixes)
      const prefixes =
        selectedFrequency === 'Annual' ? ['annual', 'yearly'] : [selectedFrequency.toLowerCase()];
      filtered = filtered.filter(series => {
        const raw = series.frequency.trim().toLowerCase();
        return backendFrequency
          ? prefixes.some(prefix => raw.startsWith(prefix))
          : series.frequency === selectedFrequency;
      });
    }

    // Relevance keeps the backend's order, as does browsing (by title, a page at a time)
    if (browsing) return filtered;
    if (sortBy === 'title') {
      filtered = [...filtered].sort((a, b) => a.title.localeCompare(b.title));
    } else if (sortBy === 'lastUpdated') {
      // Newest first; series without a date go last
      filtered = [...filtered].sort((a, b) =>
        (b.lastUpdated ?? '').localeCompare(a.lastUpdated ?? '')
      );
    }

    return filtered;
  }, [selectedFrequency, backendFrequency, sortBy, browsing, allSeries]);

  // Pagination. A search's results are paged here; a browse is paged by the backend, which
  // returns just this page.
  const totalPages = Math.ceil((pagedByBackend ? browseTotal : filteredSeries.length) / PAGE_SIZE);
  const lastPage = Math.max(totalPages, 1);
  const page = Math.min(currentPage, lastPage);
  const startIndex = (page - 1) * PAGE_SIZE;
  const paginatedSeries = pagedByBackend
    ? filteredSeries
    : filteredSeries.slice(startIndex, startIndex + PAGE_SIZE);

  // A page past the last (an old link, a refetch with fewer results, or a viewer who can read
  // fewer series than whoever shared the link) moves to the last page once the results are in,
  // and a browse then fetches that page. Adjusted during render, like the form's sync with the
  // URL above.
  const resultsSettled = Boolean(activeResult?.data) && !activeResult?.isFetching;
  const pastLastPage = resultsSettled && currentPage > lastPage;
  if (pastLastPage) setCurrentPage(lastPage);

  // Update URL parameters when filters change. `source` is written back as `selectedSource`,
  // not the resolved id: a link that arrived with a source name (an old bookmark, say) keeps
  // that same name in the URL rather than having it silently rewritten to the id it resolved
  // to — a same-site navigation is expected to keep every query parameter it arrived with.
  // Nothing is written when the URL already says the same, as it does right after the form
  // took a navigation's values above.
  // The sort and page are written only when they aren't the defaults (relevance, page 1).
  React.useEffect(() => {
    const params = new URLSearchParams();
    if (searchQuery) params.set('q', searchQuery);
    if (selectedSource) params.set('source', selectedSource);
    if (selectedFrequency) params.set('frequency', selectedFrequency);
    // A browse is always by title, so a sort isn't written while browsing
    if (sortBy !== 'relevance' && !browsing) params.set('sort', sortBy);
    if (currentPage > 1) params.set('page', String(currentPage));
    if (params.toString() === searchParams.toString()) return;
    setSearchParams(params, { replace: true, state: { explorerForm: true } });
  }, [
    searchQuery,
    selectedSource,
    selectedFrequency,
    sortBy,
    browsing,
    currentPage,
    searchParams,
    setSearchParams,
  ]);

  // A result opens with this page's URL in its history state, so the series page's links back
  // to the explorer return to these results rather than an empty explorer.
  const openSeries = (id: string) =>
    navigate(`/series/${id}`, { state: { explorerUrl: `${location.pathname}${location.search}` } });

  // Keyboard shortcuts
  React.useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.metaKey) {
        switch (event.key) {
          case 'k':
            event.preventDefault();
            searchInputRef.current?.focus();
            break;
          case '/':
            event.preventDefault();
            searchInputRef.current?.focus();
            break;
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Results are fetched as the query changes; the button and Enter send it right away
  const handleSearch = () => {
    const query = searchQuery.trim();
    if (query.length < 2 || waitingForSources) return;
    if (query === debouncedQuery) {
      searchResult?.refetch?.();
    } else {
      setDebouncedQuery(query);
    }
  };

  // Clear search
  const handleClearSearch = () => {
    setSearchQuery('');
    setSelectedSource('');
    setSelectedFrequency('');
    setCurrentPage(1);
  };

  // Export functionality. Rows carry the source and its own series id, so they can be
  // matched back to the source.
  const handleExport = (format: string) => {
    const data = filteredSeries;

    if (format === 'csv') {
      downloadCsv('economic-series.csv', buildSearchResultsCsv(data));
    } else if (format === 'json') {
      const blob = new Blob([buildSearchResultsJson(data)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'economic-series.json';
      a.click();
      URL.revokeObjectURL(url);
    }

    setExportMenuAnchor(null);
    setSnackbarMessage(`Exported ${data.length} series as ${format.toUpperCase()}`);
    setSnackbarOpen(true);
  };

  // Frequencies present in the current results
  const uniqueFrequencies = React.useMemo(() => {
    const frequencies = new Set(allSeries.map(s => s.frequency).filter(Boolean));
    if (selectedFrequency) frequencies.add(selectedFrequency);
    return Array.from(frequencies).sort();
  }, [allSeries, selectedFrequency]);

  const dateRange = (series: EconomicSeries): string | undefined => {
    if (series.startDate && series.endDate) return `${series.startDate} - ${series.endDate}`;
    if (series.startDate) return `From ${series.startDate}`;
    if (series.endDate) return `Until ${series.endDate}`;
    return undefined;
  };

  // Each card sits in a sized Grid cell: a Card placed straight in the container would take a
  // width sized by its own text instead of a column.
  const renderSeriesCard = (series: EconomicSeries) => (
    <Grid key={series.id} size={{ xs: 12, sm: 6, md: 4 }}>
      <Card
        sx={{
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
          cursor: 'pointer',
          transition: 'all 0.2s ease-in-out',
          '&:hover': {
            transform: 'translateY(-2px)',
            boxShadow: 4,
          },
        }}
        onClick={() => openSeries(series.id)}
      >
        <CardContent sx={{ flexGrow: 1 }}>
          <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
            <TrendingUpIcon color='primary' sx={{ mr: 1, mt: 0.5 }} />
            <Box sx={{ flexGrow: 1 }}>
              <Typography
                variant='h6'
                component='div'
                sx={{ fontSize: '1rem', lineHeight: 1.3, mb: 1 }}
              >
                {series.title}
              </Typography>
              <Typography
                variant='body2'
                sx={{
                  color: 'text.secondary',
                  mb: 2,
                }}
              >
                {series.description}
              </Typography>
            </Box>
          </Box>

          <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mb: 2 }}>
            {series.source && (
              <Chip
                label={series.source}
                size='small'
                color='primary'
                variant='outlined'
                title={`Data Source: ${series.source}`}
              />
            )}
            <Chip label={series.frequency} size='small' variant='outlined' />
            {series.units && <Chip label={series.units} size='small' variant='outlined' />}
          </Box>

          {dateRange(series) && (
            <Box sx={{ display: 'flex', alignItems: 'center', mt: 'auto' }}>
              <AccessTimeIcon fontSize='small' color='action' sx={{ mr: 0.5 }} />
              <Typography
                variant='caption'
                sx={{
                  color: 'text.secondary',
                }}
              >
                {dateRange(series)}
              </Typography>
            </Box>
          )}
        </CardContent>

        <CardActions sx={{ pt: 0 }}>
          <Button
            size='small'
            onClick={e => {
              e.stopPropagation();
              openSeries(series.id);
            }}
          >
            View Details
          </Button>
        </CardActions>
      </Card>
    </Grid>
  );

  return (
    <Box>
      {/* Page header */}
      <Box sx={{ mb: 4 }}>
        <Typography variant='h4' component='h1' gutterBottom>
          Series Explorer
        </Typography>
        <Typography
          variant='body1'
          sx={{
            color: 'text.secondary',
          }}
        >
          Search and explore economic time series data from the available sources
        </Typography>
      </Box>

      {/* Search and filters */}
      <Paper sx={{ p: 3, mb: 4 }}>
        <Grid container spacing={3}>
          {/* Search input */}
          <Grid size={{ xs: 12, md: 6 }}>
            <TextField
              fullWidth
              placeholder='Search economic series (e.g., GDP, unemployment, inflation)'
              value={searchQuery}
              onChange={e => {
                setSearchQuery(e.target.value);
                setCurrentPage(1);
              }}
              onKeyPress={e => e.key === 'Enter' && handleSearch()}
              inputRef={searchInputRef}
              slotProps={{
                input: {
                  startAdornment: <SearchIcon sx={{ mr: 1, color: 'text.secondary' }} />,
                  endAdornment: searchQuery && (
                    <IconButton
                      size='small'
                      aria-label='Clear search text'
                      onClick={() => {
                        setSearchQuery('');
                        setCurrentPage(1);
                      }}
                    >
                      <ClearIcon />
                    </IconButton>
                  ),
                },
              }}
            />
          </Grid>

          {/* Source filter */}
          <Grid size={{ xs: 12, sm: 6, md: 2 }}>
            <FormControl fullWidth>
              <InputLabel id='source-filter-label'>Source</InputLabel>
              <Select
                labelId='source-filter-label'
                value={sourceListed ? resolvedSourceId : ''}
                onChange={e => {
                  setSelectedSource(e.target.value);
                  setCurrentPage(1);
                }}
                label='Source'
              >
                <MenuItem value=''>All Sources</MenuItem>
                {dataSources.map(source => (
                  <MenuItem key={source.id} value={source.id}>
                    {source.name}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>

          {/* Frequency filter */}
          <Grid size={{ xs: 12, sm: 6, md: 2 }}>
            <FormControl fullWidth>
              <InputLabel id='frequency-filter-label'>Frequency</InputLabel>
              <Select
                labelId='frequency-filter-label'
                value={selectedFrequency}
                onChange={e => {
                  setSelectedFrequency(e.target.value);
                  setCurrentPage(1);
                }}
                label='Frequency'
              >
                <MenuItem value=''>All Frequencies</MenuItem>
                {uniqueFrequencies.map(frequency => (
                  <MenuItem key={frequency} value={frequency}>
                    {frequency}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>

          {/* Search button */}
          <Grid size={{ xs: 12, sm: 6, md: 2 }}>
            <Button
              fullWidth
              variant='contained'
              onClick={handleSearch}
              disabled={searchQuery.trim().length < 2 || isSearchFetching || waitingForSources}
              startIcon={isSearchFetching ? <CircularProgress size={20} /> : <SearchIcon />}
            >
              {isSearchFetching ? 'Searching...' : 'Search'}
            </Button>
          </Grid>
        </Grid>

        {/* Advanced search toggle */}
        <Box sx={{ mt: 2, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Button
            startIcon={<AdvancedIcon />}
            onClick={() => setShowAdvancedSearch(!showAdvancedSearch)}
          >
            Advanced Search
          </Button>

          {/* Export button */}
          <Button
            startIcon={<ExportIcon />}
            onClick={e => setExportMenuAnchor(e.currentTarget)}
            disabled={filteredSeries.length === 0}
          >
            Export Results
          </Button>
        </Box>

        {/* Advanced search panel */}
        <Collapse in={showAdvancedSearch}>
          <Divider sx={{ my: 2 }} />
          <Grid container spacing={3}>
            <Grid size={{ xs: 12, md: 4 }}>
              <FormControl fullWidth>
                <InputLabel id='sort-by-label'>Sort By</InputLabel>
                {/* A source's listing comes a page at a time in title order, so it can't be
                    reordered here */}
                <Select
                  labelId='sort-by-label'
                  value={browsing ? 'title' : sortBy}
                  disabled={browsing}
                  onChange={e => {
                    setSortBy(parseSortParam(e.target.value));
                    setCurrentPage(1);
                  }}
                  label='Sort By'
                >
                  <MenuItem value='relevance'>Relevance</MenuItem>
                  <MenuItem value='title'>Title</MenuItem>
                  <MenuItem value='lastUpdated'>Last Updated</MenuItem>
                </Select>
              </FormControl>
            </Grid>
          </Grid>
        </Collapse>
      </Paper>

      {/* Source list error */}
      {dataSourcesResult?.error && (
        <Alert severity='warning' sx={{ mb: 3 }}>
          Could not load the list of sources, so results can't be narrowed by source.
        </Alert>
      )}

      {/* Search error */}
      {searchError && (
        <Alert
          severity='error'
          sx={{ mb: 3 }}
          action={
            <Button
              color='inherit'
              size='small'
              disabled={activeResult?.isFetching}
              onClick={() => void activeResult?.refetch()}
            >
              Retry
            </Button>
          }
        >
          {browsing ? 'Could not list series' : 'Search failed'}:{' '}
          {searchError instanceof Error ? searchError.message : 'unknown error'}
        </Alert>
      )}

      {/* Result count */}
      {searchResults && hasQuery && !showLoading && !searchError && (
        <Alert severity='info' sx={{ mb: 3 }}>
          {browsing
            ? browseMessage(filteredSeries.length, searchResults.length, browseOffset, browseTotal)
            : searchResults.length >= SEARCH_LIMIT
              ? `Showing matches among the first ${SEARCH_LIMIT} results. Refine the search to narrow them.`
              : `Found ${filteredSeries.length.toLocaleString()} ${
                  filteredSeries.length === 1 ? 'result' : 'results'
                }`}
        </Alert>
      )}

      {/* Results */}
      {showLoading ? (
        <Grid container spacing={3}>
          {Array.from({ length: 8 }).map((_, index) => (
            <Grid key={index} size={{ xs: 12, sm: 6, md: 4 }}>
              <Skeleton variant='rectangular' height={200} />
            </Grid>
          ))}
        </Grid>
      ) : searchError ? null : !hasQuery ? (
        <Paper sx={{ p: 6, textAlign: 'center' }}>
          <Typography variant='h6' gutterBottom>
            Search for a series
          </Typography>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
            }}
          >
            Type at least 2 characters, such as GDP, unemployment or inflation, or pick a source to
            list its series
          </Typography>
        </Paper>
      ) : filteredSeries.length === 0 ? (
        <Paper sx={{ p: 6, textAlign: 'center' }}>
          <Typography variant='h6' gutterBottom>
            No series found
          </Typography>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              mb: 3,
            }}
          >
            Try adjusting your search criteria
          </Typography>
          <Button variant='contained' onClick={handleClearSearch}>
            Clear search and filters
          </Button>
        </Paper>
      ) : (
        <>
          {/* Results grid */}
          <Grid container spacing={3} sx={{ mb: 4 }}>
            {paginatedSeries.map(renderSeriesCard)}
          </Grid>

          {/* Pagination */}
          {totalPages > 1 && (
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 4 }}>
              <Pagination
                count={totalPages}
                page={page}
                onChange={(_, page) => {
                  setCurrentPage(page);
                  window.scrollTo(0, 0);
                }}
                color='primary'
                size='large'
              />
            </Box>
          )}
        </>
      )}

      {/* Export menu */}
      <Menu
        anchorEl={exportMenuAnchor}
        open={Boolean(exportMenuAnchor)}
        onClose={() => setExportMenuAnchor(null)}
      >
        <MenuItem onClick={() => handleExport('csv')}>Export as CSV</MenuItem>
        <MenuItem onClick={() => handleExport('json')}>Export as JSON</MenuItem>
      </Menu>

      {/* Snackbar for notifications */}
      <Snackbar
        open={snackbarOpen}
        autoHideDuration={6000}
        onClose={() => setSnackbarOpen(false)}
        message={snackbarMessage}
      />
    </Box>
  );
};

export default SeriesExplorer;
