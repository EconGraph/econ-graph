/**
 * Main Admin Application Component
 *
 * Features:
 * - Apollo GraphQL client integration
 * - Real-time crawler monitoring
 * - Navigation between admin sections
 * - Authentication and role-based access
 */

import React, { useState } from "react";
import {
  Box,
  Typography,
  Paper,
  Container,
  Tabs,
  Tab,
  Alert,
  CircularProgress,
} from "@mui/material";
import { ApolloProvider } from "@apollo/client/react";
import { apolloClient } from "./services/graphqlClient";
import CrawlerDashboard from "./pages/CrawlerDashboard";
import CrawlerConfig from "./pages/CrawlerConfig";
import CrawlerLogs from "./pages/CrawlerLogs";

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

function TabPanel(props: TabPanelProps) {
  const { children, value, index, ...other } = props;

  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`admin-tabpanel-${index}`}
      aria-labelledby={`admin-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

function a11yProps(index: number) {
  return {
    id: `admin-tab-${index}`,
    "aria-controls": `admin-tabpanel-${index}`,
  };
}

function App() {
  const [currentTab, setCurrentTab] = useState(0);

  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    setCurrentTab(newValue);
  };

  return (
    <ApolloProvider client={apolloClient}>
      <Container maxWidth="xl" sx={{ py: 2 }}>
        <Paper elevation={3} sx={{ p: 2 }}>
          <Typography variant="h3" component="h1" gutterBottom align="center">
            🕷️ EconGraph Crawler Administration
          </Typography>

          <Typography
            variant="subtitle1"
            align="center"
            color="text.secondary"
            sx={{ mb: 3 }}
          >
            Real-time monitoring and management of data acquisition
            infrastructure
          </Typography>

          <Box sx={{ borderBottom: 1, borderColor: "divider" }}>
            <Tabs
              value={currentTab}
              onChange={handleTabChange}
              aria-label="admin navigation tabs"
              centered
            >
              <Tab label="Dashboard" {...a11yProps(0)} />
              <Tab label="Configuration" {...a11yProps(1)} />
              <Tab label="Logs & Monitoring" {...a11yProps(2)} />
            </Tabs>
          </Box>

          <TabPanel value={currentTab} index={0}>
            <CrawlerDashboard />
          </TabPanel>

          <TabPanel value={currentTab} index={1}>
            <CrawlerConfig />
          </TabPanel>

          <TabPanel value={currentTab} index={2}>
            <CrawlerLogs />
          </TabPanel>
        </Paper>
      </Container>
    </ApolloProvider>
  );
}

export default App;
