// REQUIREMENT: Security context for admin interface access control
// PURPOSE: Provide role-based access control and security monitoring
// This ensures proper authorization and tracks security events

import React, { createContext, useContext, useEffect, useState } from 'react';
import { useAuth } from './AuthContext';

interface SecurityEvent {
  id: string;
  type: 'access_denied' | 'suspicious_activity' | 'privilege_escalation' | 'data_access' | 'config_change';
  severity: 'low' | 'medium' | 'high' | 'critical';
  message: string;
  timestamp: string;
  userId?: string;
  metadata?: Record<string, any>;
}

interface SecurityContextType {
  checkAccess: (requiredRole: string, resource?: string) => boolean;
  logSecurityEvent: (event: Omit<SecurityEvent, 'id' | 'timestamp'>) => void;
  getSecurityEvents: () => SecurityEvent[];
  isSecureConnection: boolean;
  sessionRemainingTime: number;
}

const SecurityContext = createContext<SecurityContextType | undefined>(undefined);

// Role hierarchy for access control
const ROLE_HIERARCHY = {
  'read_only': 1,
  'admin': 2,
  'super_admin': 3,
};

const ROLE_PERMISSIONS = {
  'read_only': [
    'view_dashboard',
    'view_system_health',
    'view_monitoring',
    'view_audit_logs',
  ],
  'admin': [
    'view_dashboard',
    'view_system_health',
    'view_monitoring',
    'view_audit_logs',
    'manage_crawler',
    'view_security',
    'trigger_maintenance',
  ],
  'super_admin': [
    'view_dashboard',
    'view_system_health',
    'view_monitoring',
    'view_audit_logs',
    'manage_crawler',
    'view_security',
    'trigger_maintenance',
    'manage_database',
    'manage_users',
    'manage_config',
    'view_sensitive_data',
    'system_shutdown',
  ],
};

export function SecurityProvider({ children }: { children: React.ReactNode }) {
  const { user, isAuthenticated } = useAuth();
  const [securityEvents, setSecurityEvents] = useState<SecurityEvent[]>([]);
  const [sessionRemainingTime, setSessionRemainingTime] = useState(0);
  const [isSecureConnection] = useState(() => {
    return window.location.protocol === 'https:' || window.location.hostname === 'localhost';
  });

  // Monitor session time remaining
  useEffect(() => {
    if (!user?.sessionExpiry) return;

    const updateRemainingTime = () => {
      const expiry = new Date(user.sessionExpiry).getTime();
      const remaining = Math.max(0, expiry - Date.now());
      setSessionRemainingTime(remaining);
    };

    updateRemainingTime();
    const interval = setInterval(updateRemainingTime, 1000);
    return () => clearInterval(interval);
  }, [user?.sessionExpiry]);

  // Security monitoring
  useEffect(() => {
    // Monitor for suspicious browser behavior
    const handleVisibilityChange = () => {
      if (document.hidden && isAuthenticated) {
        logSecurityEvent({
          type: 'suspicious_activity',
          severity: 'low',
          message: 'Admin interface tab became hidden',
          userId: user?.id,
        });
      }
    };

    // Monitor for developer tools
    const handleDevToolsAttempt = (e: KeyboardEvent) => {
      const isDev = (e.ctrlKey && e.shiftKey && e.key === 'I') ||
                   (e.ctrlKey && e.shiftKey && e.key === 'J') ||
                   (e.key === 'F12');

      if (isDev && process.env.NODE_ENV === 'production') {
        e.preventDefault();
        logSecurityEvent({
          type: 'suspicious_activity',
          severity: 'medium',
          message: 'Attempted to open developer tools',
          userId: user?.id,
        });
      }
    };

    // Monitor for multiple failed access attempts
    const resetFailedAttempts = () => {
      // Reset logic can be implemented here if needed
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    document.addEventListener('keydown', handleDevToolsAttempt);

    // Reset failed attempts after 5 minutes
    const resetTimer = setInterval(resetFailedAttempts, 5 * 60 * 1000);

    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      document.removeEventListener('keydown', handleDevToolsAttempt);
      clearInterval(resetTimer);
    };
  }, [isAuthenticated, user?.id]);

  const checkAccess = (requiredRole: string, resource?: string): boolean => {
    if (!isAuthenticated || !user) {
      logSecurityEvent({
        type: 'access_denied',
        severity: 'medium',
        message: `Unauthenticated access attempt to ${requiredRole}${resource ? ` for ${resource}` : ''}`,
      });
      return false;
    }

    const userRoleLevel = ROLE_HIERARCHY[user.role as keyof typeof ROLE_HIERARCHY] || 0;
    const requiredRoleLevel = ROLE_HIERARCHY[requiredRole as keyof typeof ROLE_HIERARCHY] || 0;

    // Check role hierarchy
    if (userRoleLevel < requiredRoleLevel) {
      logSecurityEvent({
        type: 'access_denied',
        severity: 'high',
        message: `Insufficient privileges: ${user.role} attempted to access ${requiredRole} resource`,
        userId: user.id,
        metadata: { requiredRole, userRole: user.role, resource },
      });
      return false;
    }

    // Check specific permissions if resource specified
    if (resource) {
      const userPermissions = ROLE_PERMISSIONS[user.role as keyof typeof ROLE_PERMISSIONS] || [];
      if (!userPermissions.includes(resource)) {
        logSecurityEvent({
          type: 'access_denied',
          severity: 'high',
          message: `Permission denied: ${user.role} attempted to access ${resource}`,
          userId: user.id,
          metadata: { resource, userPermissions },
        });
        return false;
      }
    }

    // Log successful access to sensitive resources
    if (requiredRole === 'super_admin' || resource?.includes('sensitive')) {
      logSecurityEvent({
        type: 'data_access',
        severity: 'medium',
        message: `Privileged access granted to ${resource || requiredRole}`,
        userId: user.id,
        metadata: { requiredRole, resource },
      });
    }

    return true;
  };

  const logSecurityEvent = (event: Omit<SecurityEvent, 'id' | 'timestamp'>) => {
    const securityEvent: SecurityEvent = {
      ...event,
      id: crypto.randomUUID(),
      timestamp: new Date().toISOString(),
    };

    setSecurityEvents(prev => [securityEvent, ...prev.slice(0, 999)]); // Keep last 1000 events

    // Send critical events to backend immediately
    if (event.severity === 'critical' || event.severity === 'high') {
      fetch('/api/admin/security/events', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${localStorage.getItem('admin_token')}`,
        },
        body: JSON.stringify(securityEvent),
      }).catch(console.error);
    }

    // Console log for development
    if (process.env.NODE_ENV === 'development') {
      console.warn('🔒 Security Event:', securityEvent);
    }
  };

  const getSecurityEvents = (): SecurityEvent[] => {
    return securityEvents;
  };

  // Initialize security monitoring
  useEffect(() => {
    if (isAuthenticated && user) {
      logSecurityEvent({
        type: 'data_access',
        severity: 'low',
        message: 'Admin interface accessed',
        userId: user.id,
        metadata: {
          userAgent: navigator.userAgent,
          url: window.location.href,
          secureConnection: isSecureConnection,
        },
      });
    }
  }, [isAuthenticated, user, isSecureConnection]);

  // Warn about insecure connections
  useEffect(() => {
    if (!isSecureConnection && process.env.NODE_ENV === 'production') {
      logSecurityEvent({
        type: 'suspicious_activity',
        severity: 'critical',
        message: 'Admin interface accessed over insecure connection',
        metadata: { protocol: window.location.protocol },
      });
    }
  }, [isSecureConnection]);

  const value: SecurityContextType = {
    checkAccess,
    logSecurityEvent,
    getSecurityEvents,
    isSecureConnection,
    sessionRemainingTime,
  };

  return (
    <SecurityContext.Provider value={value}>
      {children}
    </SecurityContext.Provider>
  );
}

export function useSecurity() {
  const context = useContext(SecurityContext);
  if (context === undefined) {
    throw new Error('useSecurity must be used within a SecurityProvider');
  }
  return context;
}
