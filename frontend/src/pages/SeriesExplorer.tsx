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
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useSeriesSearch, useDataSources } from '../hooks/useSeriesData';

interface EconomicSeries {
  id: string;
  title: string;
  description: string;
  sourceId: string;
  source: string;
  frequency: string;
  units: string;
  lastUpdated?: string;
  startDate?: string;
  endDate?: string;
}

/** Most results the page asks the backend for in one search. */
const SEARCH_LIMIT = 100;

/** How long typing must pause before the search is sent. */
const SEARCH_DEBOUNCE_MS = 250;

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

/**
 * Quotes a value for a CSV cell when it contains a comma, quote or newline. A leading
 * =, +, -, @, tab or CR is prefixed with a quote first, so a spreadsheet app never runs
 * a cell pulled from search results (title, description, source, units) as a formula.
 * @param value - Cell value.
 * @returns The escaped cell.
 */
const toCsvCell = (value: string | undefined): string => {
  const raw = value ?? '';
  const text = /^[=+\-@\t\r]/.test(raw) ? `'${raw}` : raw;
  return /[",\n\r]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
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
 * already an id, but older links (the sidebar, the data sources page) put the source's
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
  const [currentPage, setCurrentPage] = React.useState(1);

  // Advanced search state
  const [showAdvancedSearch, setShowAdvancedSearch] = React.useState(false);
  const [sortBy, setSortBy] = React.useState('relevance');

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
  // the URL (the sidebar, the data sources page) resolves to that source's id instead of
  // matching nothing. While the list is loading, the source from the URL is held and the
  // search waits for it.
  const resolvedSourceId = resolveSourceId(selectedSource, dataSources);
  const sourceListed = resolvedSourceId !== undefined;
  const waitingForSources = Boolean(selectedSource) && Boolean(dataSourcesResult?.isLoading);
  const activeSource = sourceListed ? resolvedSourceId : waitingForSources ? selectedSource : '';

  // Send the search once typing pauses, not on every keystroke
  const [debouncedQuery, setDebouncedQuery] = React.useState(searchQuery.trim());
  React.useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(searchQuery.trim()), SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  // Search runs on the backend; the source filter is applied there too
  const searchResult = useSeriesSearch(
    debouncedQuery,
    {
      sourceId: activeSource || undefined,
      frequency: selectedFrequency ? toBackendFrequency(selectedFrequency) : undefined,
      limit: SEARCH_LIMIT,
    },
    !waitingForSources
  );
  const {
    data: searchResults,
    isInitialLoading: isSearchLoading,
    isFetching: isSearchFetching,
    error: searchError,
  } = searchResult || {};
  const showLoading = isSearchLoading || (waitingForSources && debouncedQuery.length >= 2);

  // Transform search results to the card format. Missing fields stay missing.
  const allSeries: EconomicSeries[] = React.useMemo(() => {
    if (!searchResults) return [];

    return searchResults.map((result: any) => ({
      id: result.id,
      title: result.title,
      description: result.description || '',
      sourceId: result.sourceId,
      source:
        result.source?.name ||
        dataSources.find(ds => ds.id === result.sourceId)?.name ||
        'Unknown Source',
      frequency: result.frequency,
      units: result.units || '',
      lastUpdated: toTimestamp(result.lastUpdated),
      startDate: toDateOnly(result.startDate),
      endDate: toDateOnly(result.endDate),
    }));
  }, [searchResults, dataSources]);

  // Apply the frequency filter (the backend filters by source), and sort
  const filteredSeries = React.useMemo(() => {
    let filtered = allSeries;

    if (selectedFrequency) {
      filtered = filtered.filter(series => series.frequency === selectedFrequency);
    }

    // Relevance keeps the backend's order
    if (sortBy === 'title') {
      filtered = [...filtered].sort((a, b) => a.title.localeCompare(b.title));
    } else if (sortBy === 'lastUpdated') {
      // Newest first; series without a date go last
      filtered = [...filtered].sort((a, b) =>
        (b.lastUpdated ?? '').localeCompare(a.lastUpdated ?? '')
      );
    }

    return filtered;
  }, [selectedFrequency, sortBy, allSeries]);

  // Pagination
  const itemsPerPage = 20;
  const totalPages = Math.ceil(filteredSeries.length / itemsPerPage);
  // A refetch can return fewer results, so never stay past the last page
  const page = Math.min(currentPage, Math.max(totalPages, 1));
  const startIndex = (page - 1) * itemsPerPage;
  const endIndex = startIndex + itemsPerPage;
  const paginatedSeries = filteredSeries.slice(startIndex, endIndex);

  // Update URL parameters when filters change. `source` is written back as `selectedSource`,
  // not the resolved id: a link that arrived with a source name (the sidebar's, say) keeps
  // that same name in the URL rather than having it silently rewritten to the id it resolved
  // to — a same-site navigation is expected to keep every query parameter it arrived with.
  React.useEffect(() => {
    const params = new URLSearchParams();
    if (searchQuery) params.set('q', searchQuery);
    if (selectedSource) params.set('source', selectedSource);
    if (selectedFrequency) params.set('frequency', selectedFrequency);
    setSearchParams(params, { replace: true });
  }, [searchQuery, selectedSource, selectedFrequency, setSearchParams]);

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

  // Export functionality
  const handleExport = (format: string) => {
    const data = filteredSeries.map(series => ({
      id: series.id,
      title: series.title,
      description: series.description,
      source: series.source,
      frequency: series.frequency,
      units: series.units,
      lastUpdated: series.lastUpdated,
      startDate: series.startDate,
      endDate: series.endDate,
    }));

    if (format === 'csv') {
      const csv = [
        Object.keys(data[0] || {}).join(','),
        ...data.map(row => Object.values(row).map(toCsvCell).join(',')),
      ].join('\n');

      const blob = new Blob([csv], { type: 'text/csv' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'economic-series.csv';
      a.click();
      URL.revokeObjectURL(url);
    } else if (format === 'json') {
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
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

  const renderSeriesCard = (series: EconomicSeries) => (
    <Card
      key={series.id}
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
      onClick={() => navigate(`/series/${series.id}`)}
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
            <Typography variant='body2' color='text.secondary' sx={{ mb: 2 }}>
              {series.description}
            </Typography>
          </Box>
        </Box>

        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mb: 2 }}>
          <Chip
            label={series.source}
            size='small'
            color='primary'
            variant='outlined'
            title={`Data Source: ${series.source}`}
          />
          <Chip label={series.frequency} size='small' variant='outlined' />
          {series.units && <Chip label={series.units} size='small' variant='outlined' />}
        </Box>

        {dateRange(series) && (
          <Box sx={{ display: 'flex', alignItems: 'center', mt: 'auto' }}>
            <AccessTimeIcon fontSize='small' color='action' sx={{ mr: 0.5 }} />
            <Typography variant='caption' color='text.secondary'>
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
            navigate(`/series/${series.id}`);
          }}
        >
          View Details
        </Button>
      </CardActions>
    </Card>
  );

  return (
    <Box>
      {/* Page header */}
      <Box sx={{ mb: 4 }}>
        <Typography variant='h4' component='h1' gutterBottom>
          Series Explorer
        </Typography>
        <Typography variant='body1' color='text.secondary'>
          Search and explore economic time series data from the available sources
        </Typography>
      </Box>

      {/* Search and filters */}
      <Paper sx={{ p: 3, mb: 4 }}>
        <Grid container spacing={3}>
          {/* Search input */}
          <Grid item xs={12} md={6}>
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
              InputProps={{
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
              }}
            />
          </Grid>

          {/* Source filter */}
          <Grid item xs={12} sm={6} md={2}>
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
          <Grid item xs={12} sm={6} md={2}>
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
          <Grid item xs={12} sm={6} md={2}>
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
            <Grid item xs={12} md={4}>
              <FormControl fullWidth>
                <InputLabel id='sort-by-label'>Sort By</InputLabel>
                <Select
                  labelId='sort-by-label'
                  value={sortBy}
                  onChange={e => {
                    setSortBy(e.target.value);
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
        <Alert severity='error' sx={{ mb: 3 }}>
          Search failed: {searchError instanceof Error ? searchError.message : 'unknown error'}
        </Alert>
      )}

      {/* Result count */}
      {searchResults && debouncedQuery.length >= 2 && !showLoading && !searchError && (
        <Alert severity='info' sx={{ mb: 3 }}>
          {searchResults.length >= SEARCH_LIMIT
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
            <Grid item xs={12} sm={6} md={4} key={index}>
              <Skeleton variant='rectangular' height={200} />
            </Grid>
          ))}
        </Grid>
      ) : searchError ? null : debouncedQuery.length < 2 ? (
        <Paper sx={{ p: 6, textAlign: 'center' }}>
          <Typography variant='h6' gutterBottom>
            Search for a series
          </Typography>
          <Typography variant='body2' color='text.secondary'>
            Type at least 2 characters, such as GDP, unemployment or inflation
          </Typography>
        </Paper>
      ) : filteredSeries.length === 0 ? (
        <Paper sx={{ p: 6, textAlign: 'center' }}>
          <Typography variant='h6' gutterBottom>
            No series found
          </Typography>
          <Typography variant='body2' color='text.secondary' sx={{ mb: 3 }}>
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
                onChange={(_, page) => setCurrentPage(page)}
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
