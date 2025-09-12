import React from 'react';
import { Routes, Route } from 'react-router-dom';
import { Container, Box } from '@mui/material';

import { AuthProvider } from './contexts/AuthContext';
import Header from './components/layout/Header';
import Sidebar from './components/layout/Sidebar';
import Dashboard from './pages/Dashboard';
import SeriesExplorer from './pages/SeriesExplorer';
import SeriesDetail from './pages/SeriesDetail';
import DataSources from './pages/DataSources';
import About from './pages/About';
import ProfessionalAnalysis from './pages/ProfessionalAnalysis';
import GlobalAnalysis from './pages/GlobalAnalysis';

// Sidebar width constant - must match Sidebar.tsx
const SIDEBAR_WIDTH = 240;

/**
 * REQUIREMENT: Modern application that is easier to use than FRED
 * PURPOSE: Main application component that provides routing and layout structure
 * This creates a responsive layout with navigation that's more intuitive than FRED's interface
 */
function App() {
  const [sidebarOpen, setSidebarOpen] = React.useState(true);

  const handleSidebarToggle = () => {
    setSidebarOpen(!sidebarOpen);
  };

  return (
    <AuthProvider>
      <Box sx={{ display: 'flex', minHeight: '100vh' }}>
        {/* REQUIREMENT: Modern responsive design */}
        <Header onMenuClick={handleSidebarToggle} />

        <Sidebar open={sidebarOpen} onClose={() => setSidebarOpen(false)} />

        {/* Main content area */}
        <Box
          component='main'
          sx={{
            flexGrow: 1,
            pt: { xs: 7, sm: 8 }, // Account for header height
            pl: { sm: sidebarOpen ? `${SIDEBAR_WIDTH}px` : 0 }, // Account for sidebar when open
            transition: 'padding-left 0.3s ease',
          }}
        >
          <Container maxWidth='xl' sx={{ py: 3 }}>
            <Routes>
              {/* REQUIREMENT: Function similarly to FRED but with modern UX */}
              <Route path='/' element={<Dashboard />} />
              <Route path='/explore' element={<SeriesExplorer />} />
              <Route path='/series/:id' element={<SeriesDetail />} />
              <Route path='/sources' element={<DataSources />} />
              <Route path='/about' element={<About />} />
              <Route path='/analysis/:id?' element={<ProfessionalAnalysis />} />
              <Route path='/global' element={<GlobalAnalysis />} />
            </Routes>
          </Container>
        </Box>
      </Box>
    </AuthProvider>
  );
}

export default App;
