import React from 'react';
import {
  Box,
  Typography,
  Paper,
  Grid,
  Card,
  CardContent,
  List,
  ListItem,
  ListItemIcon,
  ListItemText,
  Chip,
  Link,
} from '@mui/material';
import {
  TrendingUp as TrendingUpIcon,
  Speed as SpeedIcon,
  Security as SecurityIcon,
  Cloud as CloudIcon,
  Code as CodeIcon,
  GitHub as GitHubIcon,
} from '@mui/icons-material';

/**
 * REQUIREMENT: About page explaining the modern application features
 * PURPOSE: Provide information about EconGraph and how it improves on FRED
 * This helps users understand the platform's capabilities and advantages
 */
const About: React.FC = () => {
  const features = [
    {
      icon: <TrendingUpIcon color='primary' />,
      title: 'Interactive Visualizations',
      description: 'Modern charts with tooltips, zoom, and real-time data transformations',
    },
    {
      icon: <SpeedIcon color='primary' />,
      title: 'Fast Performance',
      description: 'GraphQL API with efficient data loading and caching for quick responses',
    },
    {
      icon: <SecurityIcon color='primary' />,
      title: 'Data Integrity',
      description: 'Track original releases vs. revisions to understand data evolution',
    },
    {
      icon: <CloudIcon color='primary' />,
      title: 'Modern Infrastructure',
      description: 'Kubernetes deployment with monitoring and automated scaling',
    },
  ];

  const technologies = [
    { name: 'React', category: 'Frontend' },
    { name: 'TypeScript', category: 'Frontend' },
    { name: 'Material-UI', category: 'Frontend' },
    { name: 'Chart.js', category: 'Frontend' },
    { name: 'GraphQL', category: 'API' },
    { name: 'Rust', category: 'Backend' },
    { name: 'PostgreSQL', category: 'Database' },
    { name: 'Diesel ORM', category: 'Backend' },
    { name: 'Kubernetes', category: 'Infrastructure' },
    { name: 'Terraform', category: 'Infrastructure' },
    { name: 'Grafana', category: 'Monitoring' },
  ];

  const dataSources = [
    'Federal Reserve Economic Data (FRED)',
    'Bureau of Labor Statistics (BLS)',
    'U.S. Census Bureau',
    'World Bank Open Data',
  ];

  return (
    <Box>
      {/* Hero section */}
      <Paper
        sx={{
          p: 4,
          mb: 4,
          background: 'linear-gradient(135deg, #1976d2 0%, #1565c0 100%)',
          color: 'white',
        }}
      >
        <Box sx={{ textAlign: 'center' }}>
          <TrendingUpIcon sx={{ fontSize: 60, mb: 2 }} />
          <Typography variant='h3' component='h1' gutterBottom>
            EconGraph
          </Typography>
          <Typography variant='h5' sx={{ mb: 3, opacity: 0.9 }}>
            Modern Economic Data Visualization Platform
          </Typography>
          <Typography variant='body1' sx={{ maxWidth: 600, mx: 'auto', opacity: 0.9 }}>
            A next-generation economic data platform that combines the comprehensive data coverage
            of FRED with modern web technologies and intuitive user experience.
          </Typography>
          <Typography variant='body2' sx={{ mt: 2, opacity: 0.8 }}>
            Version 3.7.2
          </Typography>
          <Box sx={{ mt: 3, display: 'flex', gap: 2, justifyContent: 'center', flexWrap: 'wrap' }}>
            <Link
              href='/'
              color='inherit'
              sx={{ textDecoration: 'none', '&:hover': { textDecoration: 'underline' } }}
            >
              Dashboard
            </Link>
            <Link
              href='/explore'
              color='inherit'
              sx={{ textDecoration: 'none', '&:hover': { textDecoration: 'underline' } }}
            >
              Explore Series
            </Link>
            <Link
              href='/sources'
              color='inherit'
              sx={{ textDecoration: 'none', '&:hover': { textDecoration: 'underline' } }}
            >
              Data Sources
            </Link>
          </Box>
        </Box>
      </Paper>

      <Grid container spacing={4}>
        {/* Overview */}
        <Grid item xs={12} md={8}>
          <Paper sx={{ p: 3, mb: 3 }}>
            <Typography variant='h5' gutterBottom>
              About EconGraph
            </Typography>
            <Typography variant='body1' paragraph>
              EconGraph is a modern economic data visualization platform inspired by the Federal
              Reserve Economic Data (FRED) system. While FRED provides excellent data coverage,
              EconGraph focuses on delivering that same comprehensive data through a more intuitive,
              responsive, and feature-rich interface.
            </Typography>

            <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
              Key Improvements Over FRED
            </Typography>
            <List>
              <ListItem>
                <ListItemIcon>
                  <TrendingUpIcon color='primary' />
                </ListItemIcon>
                <ListItemText
                  primary='Interactive Charts'
                  secondary='Hover tooltips, date range selection, and real-time transformations'
                />
              </ListItem>
              <ListItem>
                <ListItemIcon>
                  <SpeedIcon color='primary' />
                </ListItemIcon>
                <ListItemText
                  primary='Modern Performance'
                  secondary='GraphQL API with efficient data loading and client-side caching'
                />
              </ListItem>
              <ListItem>
                <ListItemIcon>
                  <SecurityIcon color='primary' />
                </ListItemIcon>
                <ListItemText
                  primary='Revision Tracking'
                  secondary='Compare original releases with later revisions for better analysis'
                />
              </ListItem>
              <ListItem>
                <ListItemIcon>
                  <CloudIcon color='primary' />
                </ListItemIcon>
                <ListItemText
                  primary='Responsive Design'
                  secondary='Works seamlessly on desktop, tablet, and mobile devices'
                />
              </ListItem>
            </List>
          </Paper>

          {/* Features */}
          <Paper sx={{ p: 3 }}>
            <Typography variant='h5' gutterBottom>
              Features
            </Typography>
            <Grid container spacing={2}>
              {features.map((feature, index) => (
                <Grid item xs={12} sm={6} key={index}>
                  <Card variant='outlined'>
                    <CardContent>
                      <Box sx={{ display: 'flex', alignItems: 'flex-start', mb: 2 }}>
                        {feature.icon}
                        <Typography variant='h6' sx={{ ml: 1 }}>
                          {feature.title}
                        </Typography>
                      </Box>
                      <Typography variant='body2' color='text.secondary'>
                        {feature.description}
                      </Typography>
                    </CardContent>
                  </Card>
                </Grid>
              ))}
            </Grid>
          </Paper>
        </Grid>

        {/* Sidebar */}
        <Grid item xs={12} md={4}>
          {/* Data Sources */}
          <Paper sx={{ p: 3, mb: 3 }}>
            <Typography variant='h6' gutterBottom>
              Data Sources
            </Typography>
            <List dense>
              {dataSources.map((source, index) => (
                <ListItem key={index}>
                  <ListItemIcon>
                    <TrendingUpIcon color='primary' fontSize='small' />
                  </ListItemIcon>
                  <ListItemText primary={source} />
                </ListItem>
              ))}
            </List>
          </Paper>

          {/* Technology Stack */}
          <Paper sx={{ p: 3, mb: 3 }}>
            <Typography variant='h6' gutterBottom>
              Technology Stack
            </Typography>
            <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1 }}>
              {technologies.map((tech, index) => (
                <Chip
                  key={index}
                  label={tech.name}
                  size='small'
                  color={
                    tech.category === 'Frontend'
                      ? 'primary'
                      : tech.category === 'Backend'
                        ? 'secondary'
                        : 'default'
                  }
                  variant='outlined'
                />
              ))}
            </Box>
          </Paper>

          {/* Open Source */}
          <Paper sx={{ p: 3 }}>
            <Typography variant='h6' gutterBottom sx={{ display: 'flex', alignItems: 'center' }}>
              <CodeIcon sx={{ mr: 1 }} />
              Open Source
            </Typography>
            <Typography variant='body2' paragraph>
              EconGraph is built with modern, open-source technologies and follows best practices
              for scalability, security, and maintainability.
            </Typography>
            <Link
              href='https://github.com/econograph/econograph'
              target='_blank'
              rel='noopener noreferrer'
              sx={{ display: 'flex', alignItems: 'center', textDecoration: 'none' }}
            >
              <GitHubIcon sx={{ mr: 1 }} />
              View on GitHub
            </Link>
          </Paper>
        </Grid>
      </Grid>

      {/* Footer */}
      <Box sx={{ mt: 6, py: 3, borderTop: 1, borderColor: 'divider', textAlign: 'center' }}>
        <Typography variant='body2' color='text.secondary'>
          EconGraph is an independent project and is not affiliated with the Federal Reserve Bank of
          St. Louis or FRED.
        </Typography>
        <Typography variant='body2' color='text.secondary' sx={{ mt: 1 }}>
          Built with ❤️ for the economics and data visualization community.
        </Typography>
      </Box>
    </Box>
  );
};

export default About;
