// REQUIREMENT: Main admin application with secure routing and authentication
// PURPOSE: Provide administrative interface with proper access controls
// This ensures only authenticated administrators can access system management features

import React from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { Box } from '@mui/material';
import { useAuth } from './contexts/AuthContext';
import { useSecurity } from './contexts/SecurityContext';

// Admin-specific components
import AdminLayout from './components/layout/AdminLayout';
import LoginPage from './pages/auth/LoginPage';
import DashboardPage from './pages/DashboardPage';
import SystemHealthPage from './pages/SystemHealthPage';
import CrawlerManagementPage from './pages/CrawlerManagementPage';
import DatabaseManagementPage from './pages/DatabaseManagementPage';
import UserManagementPage from './pages/UserManagementPage';
import AuditLogsPage from './pages/AuditLogsPage';
import SystemConfigPage from './pages/SystemConfigPage';
import MonitoringPage from './pages/MonitoringPage';
import SecurityPage from './pages/SecurityPage';

// Protected route wrapper
function ProtectedRoute({ children, requiredRole = 'admin' }: {
  children: React.ReactNode;
  requiredRole?: string;
}) {
  const { isAuthenticated, user, loading } = useAuth();
  const { checkAccess } = useSecurity();

  if (loading) {
    return <div>Authenticating...</div>;
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  if (!checkAccess(requiredRole)) {
    return (
      <Box sx={{ p: 3, textAlign: 'center' }}>
        <h2>🚫 Access Denied</h2>
        <p>You do not have sufficient privileges to access this resource.</p>
        <p>Required role: {requiredRole}</p>
        <p>Your role: {user?.role || 'none'}</p>
      </Box>
    );
  }

  return <>{children}</>;
}

function App() {
  const { isAuthenticated } = useAuth();

  return (
    <Routes>
      {/* Public routes (login only) */}
      <Route
        path="/login"
        element={
          isAuthenticated ? <Navigate to="/" replace /> : <LoginPage />
        }
      />

      {/* Protected admin routes */}
      <Route path="/" element={
        <ProtectedRoute>
          <AdminLayout />
        </ProtectedRoute>
      }>
        {/* Dashboard - default admin page */}
        <Route index element={<DashboardPage />} />

        {/* System monitoring and health */}
        <Route path="health" element={<SystemHealthPage />} />
        <Route path="monitoring" element={<MonitoringPage />} />

        {/* Data management */}
        <Route path="crawler" element={<CrawlerManagementPage />} />
        <Route path="database" element={
          <ProtectedRoute requiredRole="super_admin">
            <DatabaseManagementPage />
          </ProtectedRoute>
        } />

        {/* User and security management */}
        <Route path="users" element={
          <ProtectedRoute requiredRole="super_admin">
            <UserManagementPage />
          </ProtectedRoute>
        } />
        <Route path="security" element={<SecurityPage />} />
        <Route path="audit" element={<AuditLogsPage />} />

        {/* System configuration */}
        <Route path="config" element={
          <ProtectedRoute requiredRole="super_admin">
            <SystemConfigPage />
          </ProtectedRoute>
        } />
      </Route>

      {/* Catch-all redirect */}
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

export default App;
