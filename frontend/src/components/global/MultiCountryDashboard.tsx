import React, { useState } from 'react';
import {
  Box,
  Paper,
  Typography,
  Grid,
  Card,
  CardContent,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Chip,
  Tabs,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tooltip,
} from '@mui/material';
import { TrendingUp, TrendingDown, Compare, Assessment, Public, Info } from '@mui/icons-material';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip as ChartTooltip,
  Legend,
  Filler,
} from 'chart.js';
import { Line } from 'react-chartjs-2';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  ChartTooltip,
  Legend,
  Filler
);

// Sample data for demo
interface CountryData {
  id: string;
  name: string;
  isoAlpha2: string;
  gdp: number;
  gdpGrowth: number;
  inflation: number;
  unemployment: number;
  population: number;
}

const sampleCountries: CountryData[] = [
  {
    id: '1',
    name: 'United States',
    isoAlpha2: 'US',
    gdp: 23.32,
    gdpGrowth: 2.1,
    inflation: 3.2,
    unemployment: 3.7,
    population: 331.9,
  },
  {
    id: '2',
    name: 'China',
    isoAlpha2: 'CN',
    gdp: 17.73,
    gdpGrowth: 6.1,
    inflation: 2.1,
    unemployment: 4.3,
    population: 1441.0,
  },
  {
    id: '3',
    name: 'Japan',
    isoAlpha2: 'JP',
    gdp: 4.94,
    gdpGrowth: 0.3,
    inflation: 0.2,
    unemployment: 2.8,
    population: 125.8,
  },
  {
    id: '4',
    name: 'Germany',
    isoAlpha2: 'DE',
    gdp: 4.26,
    gdpGrowth: 0.6,
    inflation: 1.4,
    unemployment: 3.2,
    population: 83.2,
  },
  {
    id: '5',
    name: 'United Kingdom',
    isoAlpha2: 'GB',
    gdp: 3.13,
    gdpGrowth: 1.4,
    inflation: 2.5,
    unemployment: 4.0,
    population: 67.9,
  },
];

const MultiCountryDashboard: React.FC = () => {
  const [selectedCountries, setSelectedCountries] = useState<string[]>(['1', '2', '3']);
  const [activeTab, setActiveTab] = useState<number>(0);

  const availableCountries = sampleCountries;

  const handleCountryChange = (event: any) => {
    setSelectedCountries(event.target.value);
  };

  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    setActiveTab(newValue);
  };

  const getSelectedCountriesData = () => {
    return availableCountries.filter(country => selectedCountries.includes(country.id));
  };

  const getComparisonChartData = (metric: 'gdp' | 'gdpGrowth' | 'inflation' | 'unemployment') => {
    const countries = getSelectedCountriesData();

    return {
      labels: countries.map(c => c.name),
      datasets: [
        {
          label: getMetricLabel(metric),
          data: countries.map(c => c[metric]),
          backgroundColor: [
            'rgba(54, 162, 235, 0.6)',
            'rgba(255, 99, 132, 0.6)',
            'rgba(255, 205, 86, 0.6)',
            'rgba(75, 192, 192, 0.6)',
            'rgba(153, 102, 255, 0.6)',
          ],
          borderColor: [
            'rgba(54, 162, 235, 1)',
            'rgba(255, 99, 132, 1)',
            'rgba(255, 205, 86, 1)',
            'rgba(75, 192, 192, 1)',
            'rgba(153, 102, 255, 1)',
          ],
          borderWidth: 2,
        },
      ],
    };
  };

  const getMetricLabel = (metric: string): string => {
    switch (metric) {
      case 'gdp':
        return 'GDP (Trillions USD)';
      case 'gdpGrowth':
        return 'GDP Growth (%)';
      case 'inflation':
        return 'Inflation Rate (%)';
      case 'unemployment':
        return 'Unemployment Rate (%)';
      default:
        return metric;
    }
  };

  const getMetricIcon = (metric: string, value: number) => {
    if (metric === 'gdpGrowth') {
      return value > 0 ? <TrendingUp color='success' /> : <TrendingDown color='error' />;
    }
    if (metric === 'inflation' || metric === 'unemployment') {
      return value > 3 ? <TrendingUp color='warning' /> : <TrendingDown color='success' />;
    }
    return <Assessment color='primary' />;
  };

  const chartOptions = {
    responsive: true,
    plugins: {
      legend: {
        position: 'top' as const,
      },
      title: {
        display: true,
        text: 'Multi-Country Economic Comparison',
      },
    },
    scales: {
      y: {
        beginAtZero: true,
      },
    },
  };

  return (
    <Box sx={{ p: 3 }}>
      {/* Behind the global_analysis_tabs flag (ECO-105); scripts/check-flag-bundle.mjs looks
          for this marker to prove the release bundle leaves this component out. */}
      <span hidden>econgraph-flag-global-analysis-tabs-compare-7c2f4a</span>
      <Typography variant='h4' gutterBottom sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
        📊 Multi-Country Economic Dashboard
        <Tooltip title='Compare economic indicators across multiple countries with interactive charts and analysis'>
          {/* titleAccess gives the icon a permanent accessible name (role="img") instead of
              SvgIcon's default aria-hidden, and tabIndex makes it reachable so the tooltip
              is still keyboard-accessible now that it isn't a button. */}
          <Info
            fontSize='small'
            color='action'
            titleAccess='Compare economic indicators across multiple countries with interactive charts and analysis'
            tabIndex={0}
            sx={{ cursor: 'help' }}
          />
        </Tooltip>
      </Typography>

      {/* Country Selection */}
      <Paper sx={{ p: 2, mb: 3 }}>
        <FormControl fullWidth>
          <InputLabel>Select Countries to Compare</InputLabel>
          <Select
            multiple
            value={selectedCountries}
            onChange={handleCountryChange}
            label='Select Countries to Compare'
            renderValue={selected => (
              <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
                {selected.map(value => {
                  const country = availableCountries.find(c => c.id === value);
                  return (
                    <Chip
                      key={value}
                      label={`${country?.name} (${country?.isoAlpha2})`}
                      size='small'
                    />
                  );
                })}
              </Box>
            )}
          >
            {availableCountries.map(country => (
              <MenuItem key={country.id} value={country.id}>
                {country.name} ({country.isoAlpha2})
              </MenuItem>
            ))}
          </Select>
        </FormControl>
      </Paper>

      {/* Tabs for different views */}
      <Paper sx={{ mb: 3 }}>
        <Tabs value={activeTab} onChange={handleTabChange} centered>
          <Tab label='Overview Cards' icon={<Assessment />} />
          <Tab label='Comparison Charts' icon={<Compare />} />
          <Tab label='Data Table' icon={<Public />} />
        </Tabs>
      </Paper>

      {/* Tab Content */}
      {activeTab === 0 && (
        <Grid container spacing={3}>
          {getSelectedCountriesData().map(country => (
            <Grid item xs={12} md={6} lg={4} key={country.id}>
              <Card>
                <CardContent>
                  <Typography
                    variant='h6'
                    gutterBottom
                    sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
                  >
                    {country.name}
                    <Chip label={country.isoAlpha2} size='small' color='primary' />
                  </Typography>

                  <Grid container spacing={2}>
                    <Grid item xs={6}>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        <Assessment color='primary' />
                        <Box>
                          <Typography variant='h6'>${country.gdp}T</Typography>
                          <Typography variant='caption' color='textSecondary'>
                            GDP
                          </Typography>
                        </Box>
                      </Box>
                    </Grid>

                    <Grid item xs={6}>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        {getMetricIcon('gdpGrowth', country.gdpGrowth)}
                        <Box>
                          <Typography variant='h6'>{country.gdpGrowth}%</Typography>
                          <Typography variant='caption' color='textSecondary'>
                            Growth
                          </Typography>
                        </Box>
                      </Box>
                    </Grid>

                    <Grid item xs={6}>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        {getMetricIcon('inflation', country.inflation)}
                        <Box>
                          <Typography variant='h6'>{country.inflation}%</Typography>
                          <Typography variant='caption' color='textSecondary'>
                            Inflation
                          </Typography>
                        </Box>
                      </Box>
                    </Grid>

                    <Grid item xs={6}>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        {getMetricIcon('unemployment', country.unemployment)}
                        <Box>
                          <Typography variant='h6'>{country.unemployment}%</Typography>
                          <Typography variant='caption' color='textSecondary'>
                            Unemployment
                          </Typography>
                        </Box>
                      </Box>
                    </Grid>
                  </Grid>
                </CardContent>
              </Card>
            </Grid>
          ))}
        </Grid>
      )}

      {activeTab === 1 && (
        <Grid container spacing={3}>
          <Grid item xs={12} md={6}>
            <Paper sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                GDP Comparison
              </Typography>
              <Line data={getComparisonChartData('gdp')} options={chartOptions} />
            </Paper>
          </Grid>

          <Grid item xs={12} md={6}>
            <Paper sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                GDP Growth Comparison
              </Typography>
              <Line data={getComparisonChartData('gdpGrowth')} options={chartOptions} />
            </Paper>
          </Grid>

          <Grid item xs={12} md={6}>
            <Paper sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                Inflation Rate Comparison
              </Typography>
              <Line data={getComparisonChartData('inflation')} options={chartOptions} />
            </Paper>
          </Grid>

          <Grid item xs={12} md={6}>
            <Paper sx={{ p: 2 }}>
              <Typography variant='h6' gutterBottom>
                Unemployment Rate Comparison
              </Typography>
              <Line data={getComparisonChartData('unemployment')} options={chartOptions} />
            </Paper>
          </Grid>
        </Grid>
      )}

      {activeTab === 2 && (
        <Paper sx={{ p: 2 }}>
          <Typography variant='h6' gutterBottom>
            Economic Indicators Comparison Table
          </Typography>
          <TableContainer>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>Country</TableCell>
                  <TableCell align='right'>GDP (Trillions USD)</TableCell>
                  <TableCell align='right'>GDP Growth (%)</TableCell>
                  <TableCell align='right'>Inflation (%)</TableCell>
                  <TableCell align='right'>Unemployment (%)</TableCell>
                  <TableCell align='right'>Population (Millions)</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {getSelectedCountriesData().map(country => (
                  <TableRow key={country.id}>
                    <TableCell component='th' scope='row'>
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        {country.name}
                        <Chip label={country.isoAlpha2} size='small' variant='outlined' />
                      </Box>
                    </TableCell>
                    <TableCell align='right'>${country.gdp}</TableCell>
                    <TableCell
                      align='right'
                      sx={{ color: country.gdpGrowth > 0 ? 'success.main' : 'error.main' }}
                    >
                      {country.gdpGrowth}%
                    </TableCell>
                    <TableCell
                      align='right'
                      sx={{ color: country.inflation > 3 ? 'warning.main' : 'text.primary' }}
                    >
                      {country.inflation}%
                    </TableCell>
                    <TableCell
                      align='right'
                      sx={{ color: country.unemployment > 5 ? 'error.main' : 'text.primary' }}
                    >
                      {country.unemployment}%
                    </TableCell>
                    <TableCell align='right'>{country.population}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Paper>
      )}
    </Box>
  );
};

export default MultiCountryDashboard;
