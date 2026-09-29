import React, { lazy, Suspense } from 'react';
import { Routes, Route } from 'react-router-dom';
import { Container, Box } from '@mui/material';

import { AuthProvider } from './contexts/AuthContext';
import { ThemeProvider } from './contexts/ThemeContext';
import Header from './components/layout/Header';
import Sidebar from './components/layout/Sidebar';
import Dashboard from './pages/Dashboard';
import SeriesExplorer from './pages/SeriesExplorer';
import SeriesDetail from './pages/SeriesDetail';
import DataSources from './pages/DataSources';
import About from './pages/About';
import GlobalAnalysis from './pages/GlobalAnalysis';
import PrivacyPolicy from './pages/PrivacyPolicy';
import NotFound from './pages/NotFound';
import AuthCallback from './pages/AuthCallback';
import { CALLBACK_PATH } from './auth/oidcConfig';
import ResetQueriesOnUserChange from './auth/ResetQueriesOnUserChange';

// Build-time flag pattern (docs/build-flags.md): read `__FLAGS__.<name>` in the condition
// and import the page lazily, so a release build without the flag contains none of its code.
const FlagCanary = __FLAGS__.build_canary ? lazy(() => import('./pages/FlagCanary')) : null;

// The financial statement viewer/dashboard demo (components/financial) is unfinished; ECO-144
// keeps it out of release builds until it ships.
const FinancialComponentsDemo = __FLAGS__.financial_components
  ? lazy(() =>
      import('./pages/FinancialComponentsDemo').then(m => ({ default: m.FinancialComponentsDemo }))
    )
  : null;

// Sidebar width constant - must match Sidebar.tsx
const SIDEBAR_WIDTH = 240;

/**
 * REQUIREMENT: Modern application that is easier to use than FRED.
 * PURPOSE: Main application component that provides routing and layout structure.
 * This creates a responsive layout with navigation that's more intuitive than FRED's interface.
 * @returns The main application component with routing and layout.
 */
function App() {
  const [sidebarOpen, setSidebarOpen] = React.useState(true);

  const handleSidebarToggle = () => {
    setSidebarOpen(!sidebarOpen);
  };

  return (
    <AuthProvider>
      <ResetQueriesOnUserChange />
      <ThemeProvider>
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
                <Route path='/global' element={<GlobalAnalysis />} />
                <Route path='/privacy' element={<PrivacyPolicy />} />
                <Route path={CALLBACK_PATH} element={<AuthCallback />} />
                {FlagCanary && (
                  <Route
                    path='/flags/canary'
                    element={
                      <Suspense fallback={null}>
                        <FlagCanary />
                      </Suspense>
                    }
                  />
                )}
                {FinancialComponentsDemo && (
                  <Route
                    path='/financial-components-demo'
                    element={
                      <Suspense fallback={null}>
                        <FinancialComponentsDemo />
                      </Suspense>
                    }
                  />
                )}
                <Route path='*' element={<NotFound />} />
              </Routes>
            </Container>
          </Box>
        </Box>
      </ThemeProvider>
    </AuthProvider>
  );
}

export default App;
