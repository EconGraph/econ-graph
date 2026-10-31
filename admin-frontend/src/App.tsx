import { useState } from "react";
import { Box, Container, Paper, Tab, Tabs, Typography } from "@mui/material";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "./lib/queryClient";
import CrawlerDashboard from "./pages/CrawlerDashboard";
import CrawlerConfig from "./pages/CrawlerConfig";
import CrawlerLogs from "./pages/CrawlerLogs";

export default function App() {
  const [tab, setTab] = useState(0);
  return (
    <QueryClientProvider client={queryClient}>
      <Container maxWidth="xl" sx={{ py: 3 }}>
        <Paper sx={{ p: 3 }}>
          <Typography variant="h3" component="h1">
            EconGraph Crawler Administration
          </Typography>
          <Typography sx={{ my: 2 }}>
            Authenticated operator access to the persistent crawler queue.
          </Typography>
          <Tabs
            value={tab}
            onChange={(_, value: number) => setTab(value)}
            aria-label="Admin navigation"
          >
            {["Dashboard", "Configuration", "Logs & Monitoring"].map(
              (label, index) => (
                <Tab
                  key={label}
                  label={label}
                  id={`admin-tab-${index}`}
                  aria-controls={`admin-panel-${index}`}
                />
              ),
            )}
          </Tabs>
          <Box
            role="tabpanel"
            id={`admin-panel-${tab}`}
            aria-labelledby={`admin-tab-${tab}`}
            sx={{ py: 3 }}
          >
            {tab === 0 ? (
              <CrawlerDashboard />
            ) : tab === 1 ? (
              <CrawlerConfig />
            ) : (
              <CrawlerLogs />
            )}
          </Box>
        </Paper>
      </Container>
    </QueryClientProvider>
  );
}
