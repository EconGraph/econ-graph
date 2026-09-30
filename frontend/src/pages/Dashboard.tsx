import React from 'react';
import { Grid, Typography, Box, Paper, Button } from '@mui/material';
import { Assessment as AssessmentIcon } from '@mui/icons-material';

import { useNavigate } from 'react-router-dom';
import { DASHBOARD_SERIES } from '../config/dashboardSeries';
import { LatestValueCard } from '../components/dashboard/LatestValueCard';

/**
 * Dashboard overview: the newest value of each series listed in
 * `config/dashboard-series.json`, and quick links into the explorer.
 * @returns JSX element representing the Dashboard page.
 */
const Dashboard: React.FC = () => {
  const navigate = useNavigate();

  return (
    <Box>
      <Box sx={{ mb: 4 }}>
        <Typography variant='h4' component='h1' gutterBottom>
          Economic Dashboard
        </Typography>
        <Typography variant='body1' color='text.secondary'>
          The latest published value of key economic indicators
        </Typography>
      </Box>

      <Grid container spacing={3}>
        <Grid item xs={12}>
          <Typography variant='h5' component='h2' sx={{ mb: 2 }}>
            Key Indicators
          </Typography>
        </Grid>

        {DASHBOARD_SERIES.map(entry => (
          <Grid item xs={12} sm={6} md={3} key={entry.id}>
            <LatestValueCard entry={entry} />
          </Grid>
        ))}

        <Grid item xs={12}>
          <Paper sx={{ p: 3 }}>
            <Typography variant='h6' component='h2' gutterBottom>
              Quick Actions
            </Typography>

            <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 2 }}>
              <Button
                variant='contained'
                startIcon={<AssessmentIcon />}
                onClick={() => navigate('/explore')}
              >
                Explore All Series
              </Button>

              <Button variant='outlined' onClick={() => navigate('/sources')}>
                Browse Data Sources
              </Button>
            </Box>
          </Paper>
        </Grid>
      </Grid>
    </Box>
  );
};

export default Dashboard;
