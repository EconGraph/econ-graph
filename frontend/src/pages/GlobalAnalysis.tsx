import React, { Suspense, lazy, useState } from 'react';
import {
  Box,
  Container,
  Typography,
  Tabs,
  Tab,
  Paper,
  Breadcrumbs,
  Link,
  Alert,
  Fade,
  useTheme,
  useMediaQuery,
} from '@mui/material';
import { Public, Timeline, CompareArrows, Assessment, Home } from '@mui/icons-material';
import { Link as RouterLink } from 'react-router-dom';

// Import our new global analysis components
import GlobalAnalysisDemo from './GlobalAnalysisDemo';

// The comparison and events tabs aren't on real data yet (MAP-6, train 1); compiled out of the
// release build until then (docs/build-flags.md). ECO-105.
const MultiCountryDashboard = __FLAGS__.global_analysis_tabs
  ? lazy(() => import('../components/global/MultiCountryDashboard'))
  : null;
const GlobalEventsExplorer = __FLAGS__.global_analysis_tabs
  ? lazy(() => import('../components/global/GlobalEventsExplorer'))
  : null;

type GlobalAnalysisTab = 'map' | 'compare' | 'events' | 'impact';

interface TabPanelProps {
  active: boolean;
  children?: React.ReactNode;
  tab: GlobalAnalysisTab;
}

function TabPanel({ active, tab, children }: TabPanelProps) {
  return (
    <div
      role='tabpanel'
      hidden={!active}
      id={`global-analysis-tabpanel-${tab}`}
      aria-labelledby={`global-analysis-tab-${tab}`}
    >
      {active && (
        <Fade in={true} timeout={500}>
          <Box sx={{ py: 3 }}>{children}</Box>
        </Fade>
      )}
    </div>
  );
}

function a11yProps(tab: GlobalAnalysisTab) {
  return {
    id: `global-analysis-tab-${tab}`,
    'aria-controls': `global-analysis-tabpanel-${tab}`,
  };
}

const GlobalAnalysis: React.FC = () => {
  const [activeTab, setActiveTab] = useState<GlobalAnalysisTab>('map');
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));

  const handleTabChange = (event: React.SyntheticEvent, newValue: GlobalAnalysisTab) => {
    setActiveTab(newValue);
  };

  return (
    <Container maxWidth={false} sx={{ py: 3 }}>
      {/* Header */}
      <Box sx={{ mb: 3 }}>
        <Breadcrumbs aria-label='breadcrumb' sx={{ mb: 2 }}>
          <Link
            component={RouterLink}
            to='/'
            color='inherit'
            sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}
          >
            <Home fontSize='small' />
            Dashboard
          </Link>
          <Typography color='text.primary' sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
            <Public fontSize='small' />
            Global Analysis
          </Typography>
        </Breadcrumbs>

        <Typography
          variant='h3'
          component='h1'
          gutterBottom
          sx={{
            background: 'linear-gradient(45deg, #1976d2, #42a5f5)',
            backgroundClip: 'text',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            fontWeight: 'bold',
          }}
        >
          🌍 Global Economic Network Analysis
        </Typography>

        <Typography variant='h6' color='text.secondary' paragraph>
          Explore economic interconnections, cross-country correlations, and global event impacts
          across the world's major economies in real-time.
        </Typography>

        <Alert severity='info' sx={{ mb: 3 }}>
          <strong>Bloomberg Terminal-Level Analysis:</strong> This platform provides
          institutional-grade global economic network analysis, correlation mapping, and
          cross-country impact assessment typically found only in premium financial terminals.
        </Alert>
      </Box>

      {/* Navigation Tabs */}
      <Paper sx={{ mb: 3 }}>
        <Tabs
          value={activeTab}
          onChange={handleTabChange}
          variant={isMobile ? 'scrollable' : 'standard'}
          scrollButtons='auto'
          sx={{ borderBottom: 1, borderColor: 'divider' }}
        >
          <Tab
            value='map'
            label='Network Map'
            icon={<Public />}
            iconPosition='start'
            {...a11yProps('map')}
            sx={{ minHeight: 64 }}
          />
          {MultiCountryDashboard && (
            <Tab
              value='compare'
              label='Multi-Country Dashboard'
              icon={<CompareArrows />}
              iconPosition='start'
              {...a11yProps('compare')}
              sx={{ minHeight: 64 }}
            />
          )}
          {GlobalEventsExplorer && (
            <Tab
              value='events'
              label='Global Events'
              icon={<Timeline />}
              iconPosition='start'
              {...a11yProps('events')}
              sx={{ minHeight: 64 }}
            />
          )}
          {__FLAGS__.global_analysis_tabs && (
            <Tab
              value='impact'
              label='Impact Analysis'
              icon={<Assessment />}
              iconPosition='start'
              {...a11yProps('impact')}
              sx={{ minHeight: 64 }}
            />
          )}
        </Tabs>
      </Paper>

      {/* Tab Content */}
      <TabPanel active={activeTab === 'map'} tab='map'>
        <Box sx={{ mb: 2 }}>
          <Typography
            variant='h5'
            gutterBottom
            sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
          >
            <Public color='primary' />
            Interactive Global Economic Network Map
          </Typography>
          <Typography variant='body1' color='text.secondary' paragraph>
            Visualize economic correlations between countries as an interactive network. Node size
            represents economic centrality, colors indicate economic health, and connections show
            correlation strength between countries.
          </Typography>
        </Box>
        <GlobalAnalysisDemo />
      </TabPanel>

      {MultiCountryDashboard && (
        <TabPanel active={activeTab === 'compare'} tab='compare'>
          <Box sx={{ mb: 2 }}>
            <Typography
              variant='h5'
              gutterBottom
              sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
            >
              <CompareArrows color='primary' />
              Multi-Country Economic Dashboard
            </Typography>
            <Typography variant='body1' color='text.secondary' paragraph>
              Compare economic indicators across multiple countries simultaneously. Analyze GDP,
              inflation, unemployment, and trade relationships with synchronized charts and
              real-time correlation analysis.
            </Typography>
          </Box>
          <Suspense fallback={null}>
            <MultiCountryDashboard />
          </Suspense>
        </TabPanel>
      )}

      {GlobalEventsExplorer && (
        <TabPanel active={activeTab === 'events'} tab='events'>
          <Box sx={{ mb: 2 }}>
            <Typography
              variant='h5'
              gutterBottom
              sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
            >
              <Timeline color='primary' />
              Global Economic Events Explorer
            </Typography>
            <Typography variant='body1' color='text.secondary' paragraph>
              Explore major global economic events and their impacts across countries. Track
              recovery patterns, impact severity, and economic contagion effects from financial
              crises, policy changes, and external shocks.
            </Typography>
          </Box>
          <Suspense fallback={null}>
            <GlobalEventsExplorer />
          </Suspense>
        </TabPanel>
      )}

      {__FLAGS__.global_analysis_tabs && (
        <TabPanel active={activeTab === 'impact'} tab='impact'>
          <Box sx={{ mb: 2 }}>
            <Typography
              variant='h5'
              gutterBottom
              sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
            >
              <Assessment color='primary' />
              Economic Impact Analysis
            </Typography>
            <Typography variant='body1' color='text.secondary' paragraph>
              Advanced economic impact analysis and predictive modeling tools.
            </Typography>
          </Box>

          {/* Placeholder for future impact analysis features */}
          <Paper sx={{ p: 4, textAlign: 'center', bgcolor: 'grey.50' }}>
            <Assessment sx={{ fontSize: 64, color: 'grey.400', mb: 2 }} />
            <Typography variant='h6' color='text.secondary' gutterBottom>
              Advanced Impact Analysis Coming Soon
            </Typography>
            <Typography variant='body1' color='text.secondary' paragraph>
              This section will feature advanced econometric modeling, impact prediction algorithms,
              and scenario analysis tools for comprehensive economic impact assessment.
            </Typography>

            <Box
              sx={{ mt: 3, display: 'flex', justifyContent: 'center', gap: 2, flexWrap: 'wrap' }}
            >
              {[
                'Econometric Modeling',
                'Scenario Analysis',
                'Impact Prediction',
                'Risk Assessment',
                'Policy Simulation',
                'Contagion Modeling',
              ].map(feature => (
                <Paper
                  key={feature}
                  sx={{
                    p: 2,
                    minWidth: 150,
                    textAlign: 'center',
                    border: '2px dashed',
                    borderColor: 'grey.300',
                    bgcolor: 'background.paper',
                  }}
                >
                  <Typography variant='body2' color='text.secondary'>
                    {feature}
                  </Typography>
                </Paper>
              ))}
            </Box>
          </Paper>
        </TabPanel>
      )}

      {/* Footer */}
      <Box sx={{ mt: 6, pt: 3, borderTop: '1px solid', borderColor: 'divider' }}>
        <Typography variant='body2' color='text.secondary' align='center'>
          <strong>EconGraph Global Analysis Platform</strong> - Professional-grade economic network
          analysis and cross-country correlation tools for institutional research and policy
          analysis.
        </Typography>
      </Box>
    </Container>
  );
};

export default GlobalAnalysis;
