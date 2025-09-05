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
  List,
  ListItem,
  ListItemText,
  IconButton,
  Autocomplete,
  Skeleton,
  Collapse,
  Divider,
  Slider,
  FormControlLabel,
  Switch,
  Menu,
  Alert,
  CircularProgress,
  Tooltip,
  Snackbar,
} from '@mui/material';
import {
  Search as SearchIcon,
  FilterList as FilterIcon,
  Bookmark as BookmarkIcon,
  TrendingUp as TrendingUpIcon,
  AccessTime as AccessTimeIcon,
  ExpandMore as ExpandMoreIcon,
  ExpandLess as ExpandLessIcon,
  FileDownload as ExportIcon,
  Tune as AdvancedIcon,
  Clear as ClearIcon,
  Info as InfoIcon,
} from '@mui/icons-material';
import { useNavigate, useSearchParams } from 'react-router-dom';

interface EconomicSeries {
  id: string;
  title: string;
  description: string;
  source: string;
  frequency: string;
  units: string;
  lastUpdated: string;
  startDate: string;
  endDate: string;
}

/**
 * REQUIREMENT: Browse and search functionality similar to FRED but more modern
 * PURPOSE: Provide comprehensive search and filtering for economic time series
 * This improves on FRED's search with better filters and modern UI patterns
 */
const SeriesExplorer: React.FC = () => {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  
  // State management
  const [searchQuery, setSearchQuery] = React.useState(searchParams.get('q') || '');
  const [selectedSource, setSelectedSource] = React.useState(searchParams.get('source') || '');
  const [selectedFrequency, setSelectedFrequency] = React.useState(searchParams.get('frequency') || '');
  const [selectedCategory, setSelectedCategory] = React.useState(searchParams.get('category') || '');
  const [currentPage, setCurrentPage] = React.useState(1);
  const [isLoading, setIsLoading] = React.useState(false);
  
  // Advanced search state
  const [showAdvancedSearch, setShowAdvancedSearch] = React.useState(false);
  const [similarityThreshold, setSimilarityThreshold] = React.useState(0.7);
  const [dateRange, setDateRange] = React.useState<[string, string]>(['', '']);
  const [sortBy, setSortBy] = React.useState('relevance');
  const [sortOrder, setSortOrder] = React.useState<'asc' | 'desc'>('desc');
  
  // Search suggestions and statistics
  const [searchSuggestions, setSearchSuggestions] = React.useState<string[]>([]);
  const [showSuggestions, setShowSuggestions] = React.useState(false);
  const [searchStats, setSearchStats] = React.useState<{
    resultCount: number;
    searchTime: number;
    hasSpellingSuggestion?: string;
  } | null>(null);
  
  // Export and UI state
  const [exportMenuAnchor, setExportMenuAnchor] = React.useState<null | HTMLElement>(null);
  const [showEmptyState, setShowEmptyState] = React.useState(false);
  const [snackbarOpen, setSnackbarOpen] = React.useState(false);
  const [snackbarMessage, setSnackbarMessage] = React.useState('');
  
  // Search input ref for keyboard shortcuts
  const searchInputRef = React.useRef<HTMLInputElement>(null);

  // Mock data - in real app this would come from GraphQL queries
  const allMockSeries: EconomicSeries[] = [
    {
      id: 'gdp-real',
      title: 'Real Gross Domestic Product',
      description: 'Inflation-adjusted measure of the value of all goods and services produced',
      source: 'Bureau of Economic Analysis',
      frequency: 'Quarterly',
      units: 'Billions of Chained 2017 Dollars',
      lastUpdated: '2024-12-15',
      startDate: '1947-01-01',
      endDate: '2024-09-30',
    },
    {
      id: 'unemployment-rate',
      title: 'Unemployment Rate',
      description: 'Percent of labor force that is unemployed',
      source: 'Bureau of Labor Statistics',
      frequency: 'Monthly',
      units: 'Percent',
      lastUpdated: '2024-12-06',
      startDate: '1948-01-01',
      endDate: '2024-11-30',
    },
    {
      id: 'cpi-all',
      title: 'Consumer Price Index for All Urban Consumers: All Items',
      description: 'Measure of average change in prices paid by urban consumers',
      source: 'Bureau of Labor Statistics',
      frequency: 'Monthly',
      units: 'Index 1982-84=100',
      lastUpdated: '2024-12-10',
      startDate: '1947-01-01',
      endDate: '2024-11-30',
    },
    {
      id: 'fed-funds-rate',
      title: 'Federal Funds Effective Rate',
      description: 'Interest rate at which banks lend to each other overnight',
      source: 'Federal Reserve Economic Data',
      frequency: 'Daily',
      units: 'Percent',
      lastUpdated: '2024-12-15',
      startDate: '1954-07-01',
      endDate: '2024-12-15',
    },
    {
      id: 'industrial-production',
      title: 'Industrial Production Index',
      description: 'Measure of real output of manufacturing, mining, and utilities',
      source: 'Federal Reserve Economic Data',
      frequency: 'Monthly',
      units: 'Index 2017=100',
      lastUpdated: '2024-12-14',
      startDate: '1919-01-01',
      endDate: '2024-11-30',
    },
  ];

  // Filter series based on search criteria
  const filteredSeries = React.useMemo(() => {
    let filtered = allMockSeries;
    
    // Apply search query filter
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(series => 
        series.title.toLowerCase().includes(query) ||
        series.description.toLowerCase().includes(query) ||
        series.source.toLowerCase().includes(query)
      );
    }
    
    // Apply source filter
    if (selectedSource && selectedSource !== 'All Sources') {
      filtered = filtered.filter(series => series.source === selectedSource);
    }
    
    // Apply frequency filter
    if (selectedFrequency && selectedFrequency !== 'All Frequencies') {
      filtered = filtered.filter(series => series.frequency === selectedFrequency);
    }
    
    // Sort results
    filtered.sort((a, b) => {
      switch (sortBy) {
        case 'title':
          return sortOrder === 'asc' ? a.title.localeCompare(b.title) : b.title.localeCompare(a.title);
        case 'lastUpdated':
          return sortOrder === 'asc' 
            ? new Date(a.lastUpdated).getTime() - new Date(b.lastUpdated).getTime()
            : new Date(b.lastUpdated).getTime() - new Date(a.lastUpdated).getTime();
        case 'relevance':
        default:
          return 0; // Keep original order for relevance
      }
    });
    
    return filtered;
  }, [searchQuery, selectedSource, selectedFrequency, sortBy, sortOrder]);

  const dataSources = ['All Sources', 'Bureau of Labor Statistics', 'Bureau of Economic Analysis', 'Federal Reserve Economic Data'];
  const frequencies = ['All Frequencies', 'Daily', 'Weekly', 'Monthly', 'Quarterly', 'Annual'];
  const categories = ['All Categories', 'Employment', 'Inflation', 'GDP & Growth', 'Interest Rates', 'Trade'];

  // Keyboard shortcuts effect
  React.useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Ctrl+K or Cmd+K to focus search
      if ((event.ctrlKey || event.metaKey) && event.key === 'k') {
        event.preventDefault();
        searchInputRef.current?.focus();
      }
      
      // Escape to clear search
      if (event.key === 'Escape' && document.activeElement === searchInputRef.current) {
        setSearchQuery('');
        searchInputRef.current?.blur();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Search suggestions effect
  React.useEffect(() => {
    if (searchQuery.length >= 2) {
      const suggestions = [
        'unemployment rate',
        'GDP growth',
        'inflation rate',
        'federal funds rate',
        'industrial production',
        'consumer price index',
        'employment',
        'interest rates'
      ].filter(suggestion => 
        suggestion.toLowerCase().includes(searchQuery.toLowerCase()) &&
        suggestion.toLowerCase() !== searchQuery.toLowerCase()
      ).slice(0, 5);
      
      setSearchSuggestions(suggestions);
      setShowSuggestions(suggestions.length > 0);
    } else {
      setShowSuggestions(false);
    }
  }, [searchQuery]);

  const handleSearch = () => {
    const startTime = Date.now();
    setIsLoading(true);
    setShowSuggestions(false);
    
    // Update URL parameters
    const params = new URLSearchParams();
    if (searchQuery) params.set('q', searchQuery);
    if (selectedSource && selectedSource !== 'All Sources') params.set('source', selectedSource);
    if (selectedFrequency && selectedFrequency !== 'All Frequencies') params.set('frequency', selectedFrequency);
    if (selectedCategory && selectedCategory !== 'All Categories') params.set('category', selectedCategory);
    
    setSearchParams(params);
    
    // Simulate API call with realistic timing
    setTimeout(() => {
      const endTime = Date.now();
      const searchTime = endTime - startTime;
      
      // Check for empty results
      const hasResults = filteredSeries.length > 0;
      setShowEmptyState(!hasResults && searchQuery.length > 0);
      
      // Set search statistics
      setSearchStats({
        resultCount: filteredSeries.length,
        searchTime,
        hasSpellingSuggestion: searchQuery === 'GDP groth' ? 'GDP growth' : undefined
      });
      
      setIsLoading(false);
    }, Math.random() * 800 + 200); // Random delay between 200-1000ms for realism
  };

  const handleExportResults = (format: 'csv' | 'json' | 'excel') => {
    setExportMenuAnchor(null);
    setSnackbarMessage(`Exporting ${filteredSeries.length} results as ${format.toUpperCase()}...`);
    setSnackbarOpen(true);
    
    // In a real app, this would trigger actual export
    setTimeout(() => {
      setSnackbarMessage(`Export completed! ${filteredSeries.length} results exported as ${format.toUpperCase()}`);
    }, 2000);
  };

  const clearAllFilters = () => {
    setSearchQuery('');
    setSelectedSource('');
    setSelectedFrequency('');
    setSelectedCategory('');
    setDateRange(['', '']);
    setSimilarityThreshold(0.7);
    setSearchParams(new URLSearchParams());
    setShowEmptyState(false);
    setSearchStats(null);
  };

  const handleSeriesClick = (seriesId: string) => {
    navigate(`/series/${seriesId}`);
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
      onClick={() => handleSeriesClick(series.id)}
    >
      <CardContent sx={{ flexGrow: 1 }}>
        <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
          <TrendingUpIcon color="primary" sx={{ mr: 1, mt: 0.5, flexShrink: 0 }} />
          <Box sx={{ flexGrow: 1, minWidth: 0 }}>
            <Typography variant="h6" component="div" sx={{ mb: 1, wordBreak: 'break-word' }}>
              {series.title}
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              {series.description}
            </Typography>
          </Box>
        </Box>

        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mb: 2 }}>
          <Chip 
            label={series.source} 
            size="small" 
            color="primary" 
            variant="outlined"
            title={`Data Source: ${series.source}`}
          />
          <Chip label={series.frequency} size="small" variant="outlined" />
          <Chip label={series.units} size="small" variant="outlined" />
        </Box>

        {/* Show Federal Reserve Economic Data info when applicable */}
        {series.source === 'Federal Reserve Economic Data' && (
          <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
            <InfoIcon fontSize="small" color="action" sx={{ mr: 0.5 }} />
            <Typography variant="caption" color="text.secondary">
              Federal Reserve Economic Data
            </Typography>
          </Box>
        )}

        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mt: 'auto' }}>
          <Box sx={{ display: 'flex', alignItems: 'center' }}>
            <AccessTimeIcon fontSize="small" color="action" sx={{ mr: 0.5 }} />
            <Typography variant="caption" color="text.secondary">
              {series.startDate} - {series.endDate}
            </Typography>
          </Box>
        </Box>
      </CardContent>

      <CardActions sx={{ pt: 0 }}>
        <Button 
          size="small" 
          startIcon={<TrendingUpIcon />}
          component="a"
          href={`/series/${series.id}`}
          onClick={(e) => {
            e.preventDefault();
            handleSeriesClick(series.id);
          }}
        >
          View Chart
        </Button>
        <IconButton size="small" aria-label="bookmark series">
          <BookmarkIcon />
        </IconButton>
      </CardActions>
    </Card>
  );

  const renderSkeletonCard = () => (
    <Card sx={{ height: '100%' }}>
      <CardContent>
        <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
          <Skeleton variant="circular" width={24} height={24} sx={{ mr: 1, mt: 0.5 }} />
          <Box sx={{ flexGrow: 1 }}>
            <Skeleton variant="text" width="80%" height={32} />
            <Skeleton variant="text" width="100%" height={48} />
          </Box>
        </Box>
        <Box sx={{ display: 'flex', gap: 1, mb: 2 }}>
          <Skeleton variant="rectangular" width={80} height={24} sx={{ borderRadius: 12 }} />
          <Skeleton variant="rectangular" width={60} height={24} sx={{ borderRadius: 12 }} />
          <Skeleton variant="rectangular" width={100} height={24} sx={{ borderRadius: 12 }} />
        </Box>
        <Skeleton variant="text" width="60%" />
      </CardContent>
    </Card>
  );

  return (
    <Box>
      {/* Page header */}
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" component="h1" gutterBottom>
          Explore Economic Series
        </Typography>
        <Typography variant="body1" color="text.secondary">
          Search and discover economic time series data from multiple sources
        </Typography>
      </Box>

      {/* Search and filters */}
      <Paper sx={{ p: 3, mb: 4 }}>
        <Grid container spacing={3} alignItems="flex-end">
          {/* Search input with autocomplete */}
          <Grid item xs={12} md={4} sx={{ position: 'relative' }}>
            <TextField
              fullWidth
              inputRef={searchInputRef}
              label="Search series"
              placeholder="e.g., unemployment, GDP, inflation..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
              onFocus={() => searchQuery.length >= 2 && setShowSuggestions(true)}
              onBlur={() => setTimeout(() => setShowSuggestions(false), 200)}
              InputProps={{
                startAdornment: <SearchIcon color="action" sx={{ mr: 1 }} />,
                endAdornment: searchQuery && (
                  <IconButton
                    size="small"
                    onClick={() => setSearchQuery('')}
                    sx={{ mr: 1 }}
                  >
                    <ClearIcon fontSize="small" />
                  </IconButton>
                ),
              }}
            />
            
            {/* Search suggestions dropdown */}
            {showSuggestions && searchSuggestions.length > 0 && (
              <Paper
                sx={{
                  position: 'absolute',
                  top: '100%',
                  left: 0,
                  right: 0,
                  zIndex: 1000,
                  mt: 1,
                  maxHeight: 200,
                  overflow: 'auto',
                }}
              >
                <List dense>
                  {searchSuggestions.map((suggestion, index) => (
                    <ListItem
                      key={index}
                      button
                      onClick={() => {
                        setSearchQuery(suggestion);
                        setShowSuggestions(false);
                        setTimeout(handleSearch, 100);
                      }}
                    >
                      <ListItemText primary={suggestion} />
                    </ListItem>
                  ))}
                </List>
              </Paper>
            )}
          </Grid>

          {/* Source filter */}
          <Grid item xs={12} sm={6} md={2}>
            <FormControl fullWidth>
              <InputLabel>Source</InputLabel>
              <Select
                value={selectedSource}
                onChange={(e) => setSelectedSource(e.target.value)}
                label="Source"
              >
                {dataSources.map((source) => (
                  <MenuItem key={source} value={source}>
                    {source}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>

          {/* Frequency filter */}
          <Grid item xs={12} sm={6} md={2}>
            <FormControl fullWidth>
              <InputLabel>Frequency</InputLabel>
              <Select
                value={selectedFrequency}
                onChange={(e) => setSelectedFrequency(e.target.value)}
                label="Frequency"
              >
                {frequencies.map((freq) => (
                  <MenuItem key={freq} value={freq}>
                    {freq}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>

          {/* Category filter */}
          <Grid item xs={12} sm={6} md={2}>
            <FormControl fullWidth>
              <InputLabel>Category</InputLabel>
              <Select
                value={selectedCategory}
                onChange={(e) => setSelectedCategory(e.target.value)}
                label="Category"
              >
                {categories.map((category) => (
                  <MenuItem key={category} value={category}>
                    {category}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>

          {/* Action buttons */}
          <Grid item xs={12} sm={6} md={2}>
            <Box sx={{ display: 'flex', gap: 1 }}>
              <Button
                variant="contained"
                size="large"
                onClick={handleSearch}
                disabled={isLoading}
                startIcon={isLoading ? <CircularProgress size={16} /> : <SearchIcon />}
                sx={{ flexGrow: 1 }}
              >
                Search
              </Button>
              <Tooltip title="Advanced Search">
                <IconButton
                  onClick={() => setShowAdvancedSearch(!showAdvancedSearch)}
                  color={showAdvancedSearch ? 'primary' : 'default'}
                >
                  <AdvancedIcon />
                </IconButton>
              </Tooltip>
            </Box>
          </Grid>
        </Grid>
        
        {/* Advanced Search Options */}
        <Collapse in={showAdvancedSearch}>
          <Divider sx={{ my: 3 }} />
          <Typography variant="h6" gutterBottom>
            Advanced Search
          </Typography>
          <Grid container spacing={3}>
            <Grid item xs={12} sm={6} md={3}>
              <FormControl fullWidth>
                <InputLabel>Sort By</InputLabel>
                <Select
                  value={sortBy}
                  onChange={(e) => setSortBy(e.target.value)}
                  label="Sort By"
                >
                  <MenuItem value="relevance">Relevance</MenuItem>
                  <MenuItem value="title">Title</MenuItem>
                  <MenuItem value="lastUpdated">Last Updated</MenuItem>
                </Select>
              </FormControl>
            </Grid>
            <Grid item xs={12} sm={6} md={3}>
              <FormControl fullWidth>
                <InputLabel>Sort Order</InputLabel>
                <Select
                  value={sortOrder}
                  onChange={(e) => setSortOrder(e.target.value as 'asc' | 'desc')}
                  label="Sort Order"
                >
                  <MenuItem value="desc">Descending</MenuItem>
                  <MenuItem value="asc">Ascending</MenuItem>
                </Select>
              </FormControl>
            </Grid>
            <Grid item xs={12} sm={6} md={3}>
              <Typography gutterBottom>Similarity Threshold</Typography>
              <Slider
                value={similarityThreshold}
                onChange={(_, value) => setSimilarityThreshold(value as number)}
                min={0.1}
                max={1.0}
                step={0.1}
                marks
                valueLabelDisplay="auto"
                valueLabelFormat={(value) => `${Math.round(value * 100)}%`}
              />
            </Grid>
            <Grid item xs={12} sm={6} md={3}>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                <Button
                  variant="outlined"
                  startIcon={<ClearIcon />}
                  onClick={clearAllFilters}
                  size="small"
                >
                  Clear Filters
                </Button>
              </Box>
            </Grid>
          </Grid>
        </Collapse>
      </Paper>

      {/* Search Statistics and Actions */}
      <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 3 }}>
        <Box>
          <Typography variant="h6" gutterBottom>
            {isLoading ? 'Searching...' : `Found ${filteredSeries.length} series`}
          </Typography>
          {searchStats && !isLoading && (
            <Typography variant="body2" color="text.secondary">
              Found {searchStats.resultCount} results in {searchStats.searchTime}ms
              {searchStats.hasSpellingSuggestion && (
                <span>
                  {' • '}Did you mean{' '}
                  <Button
                    size="small"
                    variant="text"
                    onClick={() => setSearchQuery(searchStats.hasSpellingSuggestion!)}
                    sx={{ minWidth: 'auto', p: 0, textDecoration: 'underline' }}
                  >
                    {searchStats.hasSpellingSuggestion}
                  </Button>
                  ?
                </span>
              )}
            </Typography>
          )}
        </Box>
        
        {!isLoading && filteredSeries.length > 0 && (
          <Box sx={{ display: 'flex', gap: 1 }}>
            <Button
              startIcon={<ExportIcon />}
              onClick={(e) => setExportMenuAnchor(e.currentTarget)}
              variant="outlined"
              size="small"
            >
              Export Results
            </Button>
            <Menu
              anchorEl={exportMenuAnchor}
              open={Boolean(exportMenuAnchor)}
              onClose={() => setExportMenuAnchor(null)}
            >
              <MenuItem onClick={() => handleExportResults('csv')}>
                Export as CSV
              </MenuItem>
              <MenuItem onClick={() => handleExportResults('json')}>
                Export as JSON
              </MenuItem>
              <MenuItem onClick={() => handleExportResults('excel')}>
                Export as Excel
              </MenuItem>
            </Menu>
          </Box>
        )}
      </Box>

      {/* Empty State */}
      {showEmptyState && !isLoading && (
        <Alert
          severity="info"
          sx={{ mb: 3 }}
          action={
            <Button
              color="inherit"
              size="small"
              onClick={clearAllFilters}
            >
              Clear Filters
            </Button>
          }
        >
          <Typography variant="subtitle1" gutterBottom>
            No results found
          </Typography>
          <Typography variant="body2">
            Try adjusting your search terms or clearing some filters to see more results.
          </Typography>
        </Alert>
      )}

      {/* Series grid */}
      {!showEmptyState && (
        <Grid container spacing={3}>
          {isLoading
            ? Array.from({ length: 6 }).map((_, index) => (
                <Grid item xs={12} sm={6} lg={4} key={index}>
                  {renderSkeletonCard()}
                </Grid>
              ))
            : filteredSeries.map((series) => (
                <Grid item xs={12} sm={6} lg={4} key={series.id}>
                  {renderSeriesCard(series)}
                </Grid>
              ))}
        </Grid>
      )}

      {/* Pagination */}
      {!isLoading && filteredSeries.length > 0 && !showEmptyState && (
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mt: 4 }}>
          <Typography variant="body2" color="text.secondary">
            Showing 1-{Math.min(50, filteredSeries.length)} of {filteredSeries.length} results
          </Typography>
          <Pagination
            count={Math.ceil(filteredSeries.length / 50)}
            page={currentPage}
            onChange={(_, page) => setCurrentPage(page)}
            color="primary"
            size="large"
          />
        </Box>
      )}

      {/* Snackbar for notifications */}
      <Snackbar
        open={snackbarOpen}
        autoHideDuration={4000}
        onClose={() => setSnackbarOpen(false)}
        message={snackbarMessage}
      />
    </Box>
  );
};

export default SeriesExplorer;
