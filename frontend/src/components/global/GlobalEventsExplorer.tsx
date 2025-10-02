import React, { useState, useMemo } from 'react';
import {
  Box,
  Paper,
  Typography,
  Card,
  CardContent,
  Grid,
  Chip,
  IconButton,
  Collapse,
  Alert,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Switch,
  FormControlLabel,
  Slider,
  List,
  ListItem,
  ListItemText,
  ListItemIcon,
  // Tooltip, // Unused but kept for future features
  Divider,
} from '@mui/material';
import {
  ExpandMore,
  ExpandLess,
  FilterList,
  DateRange,
  Policy,
  Nature,
  TrendingDown,
  // TrendingUp, // Unused but kept for future features
  Public,
  Schedule as TimelineIcon,
  Assessment,
  Warning,
  // Error, // Unused but kept for future features
  Info,
} from '@mui/icons-material';

// Sample data for demo purposes
interface GlobalEventData {
  event: {
    id: string;
    name: string;
    description: string;
    eventType: string;
    severity: number;
    startDate: string;
    endDate?: string;
  };
  countryImpacts: Array<{
    country: { name: string; isoCode: string };
    impactSeverity: number;
    recoveryStatus: string;
    impactDescription: string;
  }>;
  affectedCountryCount: number;
}

const sampleGlobalEvents: GlobalEventData[] = [
  {
    event: {
      id: '1',
      name: '2008 Global Financial Crisis',
      description: 'Worldwide financial crisis that began in 2007-2008',
      eventType: 'Financial Crisis',
      severity: 5,
      startDate: '2008-09-15',
      endDate: '2009-06-01',
    },
    countryImpacts: [
      {
        country: { name: 'United States', isoCode: 'US' },
        impactSeverity: 5,
        recoveryStatus: 'recovered',
        impactDescription: 'Major banking sector collapse, housing market crash',
      },
      {
        country: { name: 'United Kingdom', isoCode: 'GB' },
        impactSeverity: 4,
        recoveryStatus: 'recovered',
        impactDescription: 'Banking sector stress, economic recession',
      },
      {
        country: { name: 'Germany', isoCode: 'DE' },
        impactSeverity: 3,
        recoveryStatus: 'recovered',
        impactDescription: 'Export decline, manufacturing slowdown',
      },
    ],
    affectedCountryCount: 3,
  },
  {
    event: {
      id: '2',
      name: 'COVID-19 Pandemic',
      description: 'Global pandemic causing widespread economic disruption',
      eventType: 'Pandemic',
      severity: 5,
      startDate: '2020-03-01',
    },
    countryImpacts: [
      {
        country: { name: 'China', isoCode: 'CN' },
        impactSeverity: 4,
        recoveryStatus: 'recovering',
        impactDescription: 'Initial outbreak, supply chain disruption',
      },
      {
        country: { name: 'Italy', isoCode: 'IT' },
        impactSeverity: 5,
        recoveryStatus: 'recovering',
        impactDescription: 'Early European epicenter, severe lockdowns',
      },
      {
        country: { name: 'United States', isoCode: 'US' },
        impactSeverity: 4,
        recoveryStatus: 'recovering',
        impactDescription: 'Massive unemployment, fiscal stimulus',
      },
    ],
    affectedCountryCount: 3,
  },
  {
    event: {
      id: '3',
      name: 'Brexit',
      description: 'UK withdrawal from the European Union',
      eventType: 'Political',
      severity: 3,
      startDate: '2020-01-31',
    },
    countryImpacts: [
      {
        country: { name: 'United Kingdom', isoCode: 'GB' },
        impactSeverity: 4,
        recoveryStatus: 'ongoing',
        impactDescription: 'Trade disruption, regulatory changes',
      },
      {
        country: { name: 'Ireland', isoCode: 'IE' },
        impactSeverity: 2,
        recoveryStatus: 'ongoing',
        impactDescription: 'Border trade complications',
      },
    ],
    affectedCountryCount: 2,
  },
];

const GlobalEventsExplorer: React.FC = () => {
  const [selectedEventTypes, setSelectedEventTypes] = useState<string[]>([]);
  const [minImpactScore, setMinImpactScore] = useState<number>(1);
  const [showRecoveredCountries, setShowRecoveredCountries] = useState<boolean>(false);
  const [expandedEvents, setExpandedEvents] = useState<Set<string>>(new Set());

  const eventTypes = ['Financial Crisis', 'Pandemic', 'Political', 'Natural Disaster', 'Trade War'];

  const filteredEvents = useMemo(() => {
    return sampleGlobalEvents.filter(eventData => {
      const { event } = eventData;

      // Filter by event type
      if (selectedEventTypes.length > 0 && !selectedEventTypes.includes(event.eventType)) {
        return false;
      }

      // Filter by minimum impact score
      if (event.severity < minImpactScore) {
        return false;
      }

      return true;
    });
  }, [selectedEventTypes, minImpactScore]);

  const handleEventTypeChange = (event: any) => {
    setSelectedEventTypes(event.target.value);
  };

  const handleMinImpactChange = (event: any, newValue: number | number[]) => {
    setMinImpactScore(newValue as number);
  };

  const toggleEventExpansion = (eventId: string) => {
    const newExpanded = new Set(expandedEvents);
    if (newExpanded.has(eventId)) {
      newExpanded.delete(eventId);
    } else {
      newExpanded.add(eventId);
    }
    setExpandedEvents(newExpanded);
  };

  const getSeverityColor = (severity: number): string => {
    if (severity >= 5) return '#f44336'; // red
    if (severity >= 4) return '#ff9800'; // orange
    if (severity >= 3) return '#ffeb3b'; // yellow
    if (severity >= 2) return '#4caf50'; // green
    return '#2196f3'; // blue
  };

  const getEventIcon = (eventType: string, _severity: number) => {
    switch (eventType.toLowerCase()) {
      case 'financial crisis':
        return <TrendingDown />;
      case 'pandemic':
        return <Warning />;
      case 'political':
        return <Policy />;
      case 'natural disaster':
        return <Nature />;
      case 'trade war':
        return <Public />;
      default:
        return <Info />;
    }
  };

  const getRecoveryStatusColor = (status: string): 'success' | 'warning' | 'error' | 'info' => {
    switch (status.toLowerCase()) {
      case 'recovered':
        return 'success';
      case 'recovering':
        return 'warning';
      case 'ongoing':
        return 'error';
      default:
        return 'info';
    }
  };

  return (
    <Box sx={{ p: 3 }}>
      <Typography variant='h4' gutterBottom sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
        🌍 Global Economic Events Explorer
        <TimelineIcon />
      </Typography>

      {/* Filters */}
      <Paper sx={{ p: 2, mb: 3 }}>
        <Typography
          variant='h6'
          gutterBottom
          sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
        >
          <FilterList />
          Event Filters
        </Typography>

        <Grid container spacing={3}>
          <Grid item xs={12} md={4}>
            <FormControl fullWidth>
              <InputLabel>Event Types</InputLabel>
              <Select
                multiple
                value={selectedEventTypes}
                onChange={handleEventTypeChange}
                label='Event Types'
                renderValue={selected => (
                  <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
                    {selected.map(value => (
                      <Chip key={value} label={value} size='small' />
                    ))}
                  </Box>
                )}
              >
                {eventTypes.map(type => (
                  <MenuItem key={type} value={type}>
                    {type}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
          </Grid>

          <Grid item xs={12} md={4}>
            <Typography gutterBottom>Minimum Impact Score: {minImpactScore}</Typography>
            <Slider
              value={minImpactScore}
              onChange={handleMinImpactChange}
              min={1}
              max={5}
              step={1}
              marks
              valueLabelDisplay='auto'
            />
          </Grid>

          <Grid item xs={12} md={4}>
            <FormControlLabel
              control={
                <Switch
                  checked={showRecoveredCountries}
                  onChange={e => setShowRecoveredCountries(e.target.checked)}
                />
              }
              label='Show Recovered Countries'
            />
          </Grid>
        </Grid>
      </Paper>

      {/* Events Timeline */}
      {filteredEvents.length === 0 ? (
        <Alert severity='info'>No global economic events match the current filters.</Alert>
      ) : (
        <Box sx={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
          {filteredEvents.map((eventData, _index) => {
            const { event, countryImpacts, affectedCountryCount } = eventData;
            const isExpanded = expandedEvents.has(event.id);
            const visibleImpacts = showRecoveredCountries
              ? countryImpacts
              : countryImpacts.filter(
                  impact => impact.recoveryStatus.toLowerCase() !== 'recovered'
                );

            return (
              <Box key={event.id} sx={{ display: 'flex', alignItems: 'flex-start', gap: 2 }}>
                {/* Event Icon */}
                <Box
                  sx={{
                    bgcolor: getSeverityColor(event.severity),
                    borderRadius: '50%',
                    p: 1,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    minWidth: 48,
                    minHeight: 48,
                    mt: 1,
                  }}
                >
                  {getEventIcon(event.eventType, event.severity)}
                </Box>

                {/* Event Content */}
                <Box sx={{ flex: 1 }}>
                  <Card sx={{ mb: 2 }}>
                    <CardContent>
                      {/* Event Header */}
                      <Box
                        sx={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'flex-start',
                          mb: 2,
                        }}
                      >
                        <Box sx={{ flexGrow: 1 }}>
                          <Typography variant='h6' gutterBottom>
                            {event.name}
                          </Typography>

                          <Box sx={{ display: 'flex', gap: 1, mb: 1, flexWrap: 'wrap' }}>
                            <Chip
                              label={event.eventType}
                              size='small'
                              color='primary'
                              variant='outlined'
                            />
                            <Chip
                              label={`${event.severity} Severity`}
                              size='small'
                              sx={{
                                bgcolor: getSeverityColor(event.severity) + '20',
                                color: getSeverityColor(event.severity),
                              }}
                            />
                            <Chip
                              label={`${affectedCountryCount} Countries`}
                              size='small'
                              variant='outlined'
                            />
                          </Box>

                          <Typography variant='body2' color='textSecondary' paragraph>
                            {event.description}
                          </Typography>

                          <Typography
                            variant='body2'
                            sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
                          >
                            <DateRange fontSize='small' />
                            <strong>Start:</strong> {new Date(event.startDate).toLocaleDateString()}
                            {event.endDate && (
                              <>
                                {' • '}
                                <strong>End:</strong> {new Date(event.endDate).toLocaleDateString()}
                              </>
                            )}
                          </Typography>
                        </Box>

                        <IconButton onClick={() => toggleEventExpansion(event.id)} sx={{ ml: 1 }}>
                          {isExpanded ? <ExpandLess /> : <ExpandMore />}
                        </IconButton>
                      </Box>

                      {/* Country Impact Summary */}
                      <Box sx={{ display: 'flex', gap: 2, mb: 2, flexWrap: 'wrap' }}>
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                          <Public fontSize='small' />
                          <Typography variant='body2'>
                            <strong>{affectedCountryCount}</strong> countries affected
                          </Typography>
                        </Box>
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                          <Assessment fontSize='small' />
                          <Typography variant='body2'>
                            <strong>
                              {countryImpacts.filter(i => i.recoveryStatus === 'recovered').length}
                            </strong>{' '}
                            recovered
                          </Typography>
                        </Box>
                      </Box>

                      {/* Expandable Country Impacts */}
                      <Collapse in={isExpanded}>
                        <Divider sx={{ mb: 2 }} />
                        <Typography variant='h6' gutterBottom>
                          Country-Specific Impacts
                        </Typography>

                        <Box sx={{ maxHeight: 300, overflow: 'auto' }}>
                          <List dense>
                            {visibleImpacts.map((impact, impactIndex) => (
                              <ListItem key={impactIndex} sx={{ pl: 0 }}>
                                <ListItemIcon>
                                  <Chip
                                    label={impact.country.isoCode}
                                    size='small'
                                    color={getRecoveryStatusColor(impact.recoveryStatus)}
                                    variant='outlined'
                                  />
                                </ListItemIcon>
                                <ListItemText
                                  primary={
                                    <Box
                                      sx={{
                                        display: 'flex',
                                        justifyContent: 'space-between',
                                        alignItems: 'center',
                                      }}
                                    >
                                      <Typography variant='body2' fontWeight='medium'>
                                        {impact.country.name}
                                      </Typography>
                                      <Box sx={{ display: 'flex', gap: 1 }}>
                                        <Chip
                                          label={`Impact: ${impact.impactSeverity}/5`}
                                          size='small'
                                          sx={{
                                            bgcolor: getSeverityColor(impact.impactSeverity) + '20',
                                            color: getSeverityColor(impact.impactSeverity),
                                          }}
                                        />
                                        <Chip
                                          label={impact.recoveryStatus}
                                          size='small'
                                          color={getRecoveryStatusColor(impact.recoveryStatus)}
                                        />
                                      </Box>
                                    </Box>
                                  }
                                  secondary={impact.impactDescription}
                                />
                              </ListItem>
                            ))}
                          </List>
                        </Box>
                      </Collapse>
                    </CardContent>
                  </Card>
                </Box>
              </Box>
            );
          })}
        </Box>
      )}

      {/* Global Impact Statistics */}
      <Paper sx={{ p: 2, mt: 3 }}>
        <Typography
          variant='h6'
          gutterBottom
          sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
        >
          <Assessment />
          Global Impact Statistics
        </Typography>

        <Grid container spacing={3}>
          <Grid item xs={12} md={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h4' color='primary'>
                {filteredEvents.length}
              </Typography>
              <Typography variant='body2' color='textSecondary'>
                Major Economic Events
              </Typography>
            </Box>
          </Grid>

          <Grid item xs={12} md={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h4' color='warning.main'>
                {filteredEvents.reduce((sum, e) => sum + e.affectedCountryCount, 0)}
              </Typography>
              <Typography variant='body2' color='textSecondary'>
                Total Country Impacts
              </Typography>
            </Box>
          </Grid>

          <Grid item xs={12} md={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h4' color='error.main'>
                {Math.round(
                  (filteredEvents.reduce((sum, e) => sum + e.event.severity, 0) /
                    filteredEvents.length) *
                    10
                ) / 10 || 0}
              </Typography>
              <Typography variant='body2' color='textSecondary'>
                Average Severity
              </Typography>
            </Box>
          </Grid>

          <Grid item xs={12} md={3}>
            <Box sx={{ textAlign: 'center' }}>
              <Typography variant='h4' color='success.main'>
                {filteredEvents.reduce(
                  (sum, e) =>
                    sum + e.countryImpacts.filter(i => i.recoveryStatus === 'recovered').length,
                  0
                )}
              </Typography>
              <Typography variant='body2' color='textSecondary'>
                Countries Recovered
              </Typography>
            </Box>
          </Grid>
        </Grid>
      </Paper>
    </Box>
  );
};

export default GlobalEventsExplorer;
