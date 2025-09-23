// Simple admin interface - minimal working version
import React from 'react';
import { Box, Typography, Paper, Container } from '@mui/material';

function App() {
  return (
    <Container maxWidth="lg" sx={{ py: 4 }}>
      <Paper elevation={3} sx={{ p: 4 }}>
        <Typography variant="h3" component="h1" gutterBottom align="center">
          🔒 EconGraph Admin Interface
        </Typography>

        <Box sx={{ mt: 4 }}>
          <Typography variant="h5" gutterBottom>
            System Status
          </Typography>

          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, mt: 2 }}>
            <Box sx={{ p: 2, bgcolor: 'success.light', borderRadius: 1 }}>
              <Typography variant="h6">✅ Admin UI Successfully Deployed</Typography>
              <Typography variant="body2">
                The admin interface is now integrated with Kubernetes infrastructure
              </Typography>
            </Box>

            <Box sx={{ p: 2, bgcolor: 'info.light', borderRadius: 1 }}>
              <Typography variant="h6">📊 Available Services</Typography>
              <Typography variant="body2">
                • Main Frontend: <a href="http://localhost:8080/" target="_blank" rel="noreferrer">http://localhost:8080/</a><br/>
                • Grafana Monitoring: <a href="http://localhost:8080/grafana" target="_blank" rel="noreferrer">http://localhost:8080/grafana</a><br/>
                • Backend API: <a href="http://localhost:8080/api" target="_blank" rel="noreferrer">http://localhost:8080/api</a><br/>
                • GraphQL Playground: <a href="http://localhost:8080/playground" target="_blank" rel="noreferrer">http://localhost:8080/playground</a>
              </Typography>
            </Box>

            <Box sx={{ p: 2, bgcolor: 'warning.light', borderRadius: 1 }}>
              <Typography variant="h6">🚧 Development Status</Typography>
              <Typography variant="body2">
                This is a minimal admin interface. Full authentication and management features are being developed.
              </Typography>
            </Box>
          </Box>
        </Box>
      </Paper>
    </Container>
  );
}

export default App;
