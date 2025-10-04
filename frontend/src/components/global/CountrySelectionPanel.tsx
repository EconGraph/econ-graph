/**
 * CountrySelectionPanel Component.
 *
 * Sidebar panel showing selected countries with their economic data
 * and providing selection management tools.
 */

import React from 'react';
import {
  Box,
  Paper,
  Typography,
  List,
  ListItem,
  ListItemText,
  ListItemSecondaryAction,
  IconButton,
  Chip,
  Button,
  Divider,
  Tooltip,
  Avatar,
  Badge,
  Menu,
  MenuItem,
  FormControl,
  InputLabel,
  Select,
} from '@mui/material';
import { Close, Clear, Compare, Download, FilterList, Sort, MoreVert } from '@mui/icons-material';
import { CountryData } from '../../types/globalAnalysis';

interface CountrySelectionPanelProps {
  /** Array of selected countries */
  selectedCountries: CountryData[];
  /** Currently selected indicator */
  selectedIndicator: string;
  /** Available indicators */
  availableIndicators: string[];
  /** Callback when a country is removed from selection */
  onRemoveCountry: (countryId: string) => void;
  /** Callback when all countries are cleared */
  onClearSelection: () => void;
  /** Callback when countries are compared */
  onCompareCountries: () => void;
  /** Callback when data is exported */
  onExportData: () => void;
  /** Callback when indicator changes */
  onIndicatorChange: (indicator: string) => void;
  /** Whether panel is visible */
  visible?: boolean;
  /** Maximum number of countries that can be selected */
  maxSelection?: number;
}

const CountrySelectionPanel: React.FC<CountrySelectionPanelProps> = ({
  selectedCountries,
  selectedIndicator,
  availableIndicators,
  onRemoveCountry,
  onClearSelection,
  onCompareCountries,
  onExportData,
  onIndicatorChange,
  visible = true,
  maxSelection = 10,
}) => {
  const [sortBy, setSortBy] = React.useState<string>('name');
  const [filterBy, setFilterBy] = React.useState<string>('all');
  const [anchorEl, setAnchorEl] = React.useState<null | HTMLElement>(null);

  const getCountryValue = React.useCallback(
    (country: CountryData) => {
      const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
      return indicator?.value || 0;
    },
    [selectedIndicator]
  );

  const getCountryUnit = React.useCallback(
    (country: CountryData) => {
      const indicator = country.economicIndicators?.find(ind => ind.name === selectedIndicator);
      return indicator?.unit || '';
    },
    [selectedIndicator]
  );

  const formatValue = React.useCallback((value: number, unit: string) => {
    if (value >= 1e12) {
      return `${(value / 1e12).toFixed(1)}T ${unit}`;
    } else if (value >= 1e9) {
      return `${(value / 1e9).toFixed(1)}B ${unit}`;
    } else if (value >= 1e6) {
      return `${(value / 1e6).toFixed(1)}M ${unit}`;
    } else if (value >= 1e3) {
      return `${(value / 1e3).toFixed(1)}K ${unit}`;
    } else {
      return `${value.toFixed(1)} ${unit}`;
    }
  }, []);

  const sortedCountries = React.useMemo(() => {
    const sorted = [...selectedCountries];

    switch (sortBy) {
      case 'name':
        return sorted.sort((a, b) => a.name.localeCompare(b.name));
      case 'value':
        return sorted.sort((a, b) => getCountryValue(b) - getCountryValue(a));
      case 'region':
        return sorted.sort((a, b) => (a.region ?? '').localeCompare(b.region ?? ''));
      default:
        return sorted;
    }
  }, [selectedCountries, sortBy, getCountryValue]);

  const filteredCountries = React.useMemo(() => {
    if (filterBy === 'all') return sortedCountries;
    return sortedCountries.filter(country => country.region === filterBy);
  }, [sortedCountries, filterBy]);

  const regions = React.useMemo(() => {
    const uniqueRegions = Array.from(
      new Set(selectedCountries.map(country => country.region).filter((r): r is string => !!r))
    );
    return uniqueRegions;
  }, [selectedCountries]);

  if (!visible) return null;

  const handleMenuOpen = (event: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(event.currentTarget);
  };

  const handleMenuClose = () => {
    setAnchorEl(null);
  };

  const canCompare = selectedCountries.length >= 2 && selectedCountries.length <= 4;
  const canExport = selectedCountries.length > 0;

  return (
    <Paper
      elevation={3}
      sx={{
        p: 2,
        minWidth: 320,
        maxWidth: 400,
        bgcolor: 'background.paper',
        borderRadius: 2,
        height: 'fit-content',
        maxHeight: '80vh',
        overflow: 'auto',
      }}
    >
      {/* Header */}
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
        <Typography variant='h6'>Selected Countries</Typography>
        <Badge badgeContent={selectedCountries.length} color='primary'>
          <Avatar sx={{ width: 32, height: 32, bgcolor: 'primary.main' }}>
            {selectedCountries.length}
          </Avatar>
        </Badge>
      </Box>

      {/* Controls */}
      <Box sx={{ display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap' }}>
        <FormControl size='small' sx={{ minWidth: 120 }}>
          <InputLabel>Sort By</InputLabel>
          <Select value={sortBy} onChange={e => setSortBy(e.target.value)} label='Sort By'>
            <MenuItem value='name'>Name</MenuItem>
            <MenuItem value='value'>Value</MenuItem>
            <MenuItem value='region'>Region</MenuItem>
          </Select>
        </FormControl>

        <FormControl size='small' sx={{ minWidth: 120 }}>
          <InputLabel>Filter</InputLabel>
          <Select value={filterBy} onChange={e => setFilterBy(e.target.value)} label='Filter'>
            <MenuItem value='all'>All Regions</MenuItem>
            {regions.map(region => (
              <MenuItem key={region} value={region}>
                {region}
              </MenuItem>
            ))}
          </Select>
        </FormControl>

        <IconButton onClick={handleMenuOpen} size='small'>
          <MoreVert />
        </IconButton>
      </Box>

      {/* Action Buttons */}
      <Box sx={{ display: 'flex', gap: 1, mb: 2, flexWrap: 'wrap' }}>
        <Button
          variant='outlined'
          size='small'
          startIcon={<Clear />}
          onClick={onClearSelection}
          disabled={selectedCountries.length === 0}
        >
          Clear All
        </Button>

        <Button
          variant='outlined'
          size='small'
          startIcon={<Compare />}
          onClick={onCompareCountries}
          disabled={!canCompare}
        >
          Compare
        </Button>

        <Button
          variant='outlined'
          size='small'
          startIcon={<Download />}
          onClick={onExportData}
          disabled={!canExport}
        >
          Export
        </Button>
      </Box>

      <Divider sx={{ my: 2 }} />

      {/* Country List */}
      <List dense>
        {filteredCountries.map(country => {
          const value = getCountryValue(country);
          const unit = getCountryUnit(country);

          return (
            <ListItem
              key={country.id}
              sx={{
                border: '1px solid #e0e0e0',
                borderRadius: 1,
                mb: 1,
                bgcolor: 'background.paper',
              }}
            >
              <ListItemText
                primary={
                  <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                    <Typography variant='subtitle2'>{country.name}</Typography>
                    <Chip label={country.isoAlpha2} size='small' variant='outlined' />
                  </Box>
                }
                secondary={
                  <Box>
                    <Typography variant='body2' color='text.secondary'>
                      {country.region}
                    </Typography>
                    <Typography variant='body2' fontWeight='bold'>
                      {formatValue(value, unit)}
                    </Typography>
                  </Box>
                }
              />
              <ListItemSecondaryAction>
                <IconButton edge='end' onClick={() => onRemoveCountry(country.id)} size='small'>
                  <Close />
                </IconButton>
              </ListItemSecondaryAction>
            </ListItem>
          );
        })}
      </List>

      {/* Empty State */}
      {selectedCountries.length === 0 && (
        <Box sx={{ textAlign: 'center', py: 4 }}>
          <Typography variant='body2' color='text.secondary'>
            No countries selected
          </Typography>
          <Typography variant='caption' display='block' sx={{ mt: 1 }}>
            Click on countries in the map to select them
          </Typography>
        </Box>
      )}

      {/* Selection Limit Warning */}
      {selectedCountries.length >= maxSelection && (
        <Box sx={{ mt: 2, p: 1, bgcolor: 'warning.light', borderRadius: 1 }}>
          <Typography variant='caption' color='warning.contrastText'>
            Maximum {maxSelection} countries selected
          </Typography>
        </Box>
      )}

      {/* Context Menu */}
      <Menu anchorEl={anchorEl} open={Boolean(anchorEl)} onClose={handleMenuClose}>
        <MenuItem onClick={handleMenuClose}>
          <FilterList sx={{ mr: 1 }} />
          Filter Options
        </MenuItem>
        <MenuItem onClick={handleMenuClose}>
          <Sort sx={{ mr: 1 }} />
          Sort Options
        </MenuItem>
      </Menu>
    </Paper>
  );
};

export default CountrySelectionPanel;
