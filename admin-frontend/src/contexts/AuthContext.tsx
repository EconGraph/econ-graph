// REQUIREMENT: Authentication context for admin interface
// PURPOSE: Provide secure authentication and session management for administrators
// This ensures only authorized personnel can access administrative functions

import React, { createContext, useContext, useReducer, useEffect } from 'react';

interface User {
  id: string;
  username: string;
  email: string;
  role: 'admin' | 'super_admin' | 'read_only';
  permissions: string[];
  lastLogin: string;
  sessionExpiry: string;
}

interface AuthState {
  user: User | null;
  isAuthenticated: boolean;
  loading: boolean;
  sessionWarning: boolean;
}

type AuthAction =
  | { type: 'LOGIN_START' }
  | { type: 'LOGIN_SUCCESS'; payload: User }
  | { type: 'LOGIN_FAILURE' }
  | { type: 'LOGOUT' }
  | { type: 'SESSION_WARNING'; payload: boolean }
  | { type: 'REFRESH_TOKEN'; payload: User };

const initialState: AuthState = {
  user: null,
  isAuthenticated: false,
  loading: true,
  sessionWarning: false,
};

function authReducer(state: AuthState, action: AuthAction): AuthState {
  switch (action.type) {
    case 'LOGIN_START':
      return { ...state, loading: true };

    case 'LOGIN_SUCCESS':
      return {
        ...state,
        user: action.payload,
        isAuthenticated: true,
        loading: false,
        sessionWarning: false,
      };

    case 'LOGIN_FAILURE':
      return {
        ...state,
        user: null,
        isAuthenticated: false,
        loading: false,
      };

    case 'LOGOUT':
      return {
        ...initialState,
        loading: false,
      };

    case 'SESSION_WARNING':
      return {
        ...state,
        sessionWarning: action.payload,
      };

    case 'REFRESH_TOKEN':
      return {
        ...state,
        user: action.payload,
        sessionWarning: false,
      };

    default:
      return state;
  }
}

interface AuthContextType extends AuthState {
  login: (username: string, password: string, mfaCode?: string) => Promise<void>;
  logout: () => void;
  refreshSession: () => Promise<void>;
  extendSession: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(authReducer, initialState);

  // Check for existing session on mount
  useEffect(() => {
    const checkExistingSession = async () => {
      try {
        const token = localStorage.getItem('admin_token');
        if (!token) {
          dispatch({ type: 'LOGIN_FAILURE' });
          return;
        }

        // Validate token with backend
        const response = await fetch('/api/admin/auth/validate', {
          headers: {
            'Authorization': `Bearer ${token}`,
          },
        });

        if (response.ok) {
          const user = await response.json();
          dispatch({ type: 'LOGIN_SUCCESS', payload: user });
          startSessionMonitoring(user);
        } else {
          localStorage.removeItem('admin_token');
          dispatch({ type: 'LOGIN_FAILURE' });
        }
      } catch (error) {
        console.error('Session validation failed:', error);
        localStorage.removeItem('admin_token');
        dispatch({ type: 'LOGIN_FAILURE' });
      }
    };

    checkExistingSession();
  }, []);

  // Session monitoring and warnings
  const startSessionMonitoring = (user: User) => {
    const sessionExpiry = new Date(user.sessionExpiry).getTime();
    const now = Date.now();
    const timeToExpiry = sessionExpiry - now;

    // Warn 5 minutes before expiry
    const warningTime = timeToExpiry - (5 * 60 * 1000);

    if (warningTime > 0) {
      setTimeout(() => {
        dispatch({ type: 'SESSION_WARNING', payload: true });
      }, warningTime);
    }

    // Auto-logout at expiry
    if (timeToExpiry > 0) {
      setTimeout(() => {
        logout();
      }, timeToExpiry);
    }
  };

  const login = async (username: string, password: string, mfaCode?: string) => {
    dispatch({ type: 'LOGIN_START' });

    try {
      // REQUIREMENT: Secure admin authentication with MFA support
      const response = await fetch('/api/admin/auth/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          username,
          password,
          mfaCode,
          clientInfo: {
            userAgent: navigator.userAgent,
            ipAddress: 'client-side-unknown', // Backend will capture real IP
            timestamp: new Date().toISOString(),
          },
        }),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Authentication failed');
      }

      const { user, token } = await response.json();

      // Store token securely
      localStorage.setItem('admin_token', token);

      // Log successful login
      console.log('🔒 Admin login successful', {
        user: user.username,
        role: user.role,
        timestamp: new Date().toISOString(),
      });

      dispatch({ type: 'LOGIN_SUCCESS', payload: user });
      startSessionMonitoring(user);

    } catch (error) {
      console.error('🚨 Admin login failed:', error);
      dispatch({ type: 'LOGIN_FAILURE' });
      throw error;
    }
  };

  const logout = () => {
    // REQUIREMENT: Secure logout with session cleanup
    const token = localStorage.getItem('admin_token');

    if (token) {
      // Notify backend of logout
      fetch('/api/admin/auth/logout', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
        },
      }).catch(console.error);
    }

    // Clear local storage
    localStorage.removeItem('admin_token');

    // Log logout
    console.log('🔒 Admin logout', {
      timestamp: new Date().toISOString(),
    });

    dispatch({ type: 'LOGOUT' });
  };

  const refreshSession = async () => {
    try {
      const token = localStorage.getItem('admin_token');
      if (!token) {
        throw new Error('No token available');
      }

      const response = await fetch('/api/admin/auth/refresh', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
        },
      });

      if (!response.ok) {
        throw new Error('Token refresh failed');
      }

      const { user, token: newToken } = await response.json();
      localStorage.setItem('admin_token', newToken);

      dispatch({ type: 'REFRESH_TOKEN', payload: user });
      startSessionMonitoring(user);

    } catch (error) {
      console.error('Session refresh failed:', error);
      logout();
    }
  };

  const extendSession = async () => {
    try {
      const token = localStorage.getItem('admin_token');
      if (!token) {
        throw new Error('No token available');
      }

      const response = await fetch('/api/admin/auth/extend', {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
        },
      });

      if (!response.ok) {
        throw new Error('Session extension failed');
      }

      const { user } = await response.json();
      dispatch({ type: 'REFRESH_TOKEN', payload: user });
      startSessionMonitoring(user);

    } catch (error) {
      console.error('Session extension failed:', error);
      throw error;
    }
  };

  const value: AuthContextType = {
    ...state,
    login,
    logout,
    refreshSession,
    extendSession,
  };

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
}
