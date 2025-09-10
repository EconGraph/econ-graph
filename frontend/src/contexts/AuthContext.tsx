/**
 * REQUIREMENT: OAuth authentication system for multi-user collaboration
 * PURPOSE: Provide secure authentication with Google and Facebook OAuth backends
 * This enables professional chart collaboration with proper user management
 */

import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
// Google Auth will be handled through backend API calls

export interface User {
  id: string;
  email: string;
  name: string;
  avatar?: string;
  provider: 'google' | 'facebook' | 'email';
  role: 'admin' | 'analyst' | 'viewer';
  organization?: string;
  preferences: {
    theme: 'light' | 'dark';
    defaultChartType: string;
    notifications: boolean;
    collaborationEnabled: boolean;
  };
  createdAt: string;
  lastLoginAt: string;
}

export interface AuthState {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  error: string | null;
}

interface AuthContextType extends AuthState {
  signInWithGoogle: () => Promise<void>;
  signInWithFacebook: () => Promise<void>;
  signInWithEmail: (email: string, password: string) => Promise<void>;
  signUp: (email: string, password: string, name: string) => Promise<void>;
  signOut: () => Promise<void>;
  updateProfile: (updates: Partial<User>) => Promise<void>;
  refreshUser: () => Promise<void>;
  clearError: () => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

// OAuth Configuration
const GOOGLE_CLIENT_ID = process.env.REACT_APP_GOOGLE_CLIENT_ID || '';
const FACEBOOK_APP_ID = process.env.REACT_APP_FACEBOOK_APP_ID || '';
const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:8080';

// Facebook SDK initialization
declare global {
  interface Window {
    FB: any;
    fbAsyncInit: () => void;
  }
}

const initFacebookSDK = () => {
  return new Promise<void>((resolve, reject) => {
    // Check if Facebook App ID is configured
    if (!FACEBOOK_APP_ID || FACEBOOK_APP_ID.trim() === '') {
      reject(new Error('Facebook App ID not configured'));
      return;
    }

    // Set a timeout for Facebook SDK initialization
    const timeout = setTimeout(() => {
      reject(new Error('Facebook SDK initialization timeout'));
    }, 10000); // 10 second timeout

    window.fbAsyncInit = () => {
      try {
        window.FB.init({
          appId: FACEBOOK_APP_ID,
          cookie: true,
          xfbml: true,
          version: 'v18.0',
        });
        clearTimeout(timeout);
        resolve();
      } catch (error) {
        clearTimeout(timeout);
        reject(new Error('Facebook SDK initialization failed'));
      }
    };

    // Load Facebook SDK with error handling
    if (!document.getElementById('facebook-jssdk')) {
      const script = document.createElement('script');
      script.id = 'facebook-jssdk';
      script.src = 'https://connect.facebook.net/en_US/sdk.js';
      script.onerror = () => {
        clearTimeout(timeout);
        reject(new Error('Failed to load Facebook SDK'));
      };
      document.head.appendChild(script);
    } else {
      // SDK already loaded, check if FB is available
      if (window.FB) {
        clearTimeout(timeout);
        resolve();
      }
    }
  });
};

export const AuthProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [authState, setAuthState] = useState<AuthState>({
    user: null,
    isAuthenticated: false,
    isLoading: true,
    error: null,
  });

  // Define refreshUser function before useEffect that uses it
  const refreshUser = useCallback(async () => {
    try {
      const token = localStorage.getItem('auth_token');
      if (!token) {
        setAuthState(prev => ({ ...prev, isLoading: false }));
        return;
      }

      const response = await fetch(`${API_BASE_URL}/auth/me`, {
        headers: {
          Authorization: `Bearer ${token}`,
        },
      });

      if (!response.ok) {
        localStorage.removeItem('auth_token');
        setAuthState({
          user: null,
          isAuthenticated: false,
          isLoading: false,
          error: null,
        });
        return;
      }

      const userData = await response.json();

      setAuthState({
        user: userData.user,
        isAuthenticated: true,
        isLoading: false,
        error: null,
      });
    } catch (error: any) {
      localStorage.removeItem('auth_token');
      setAuthState({
        user: null,
        isAuthenticated: false,
        isLoading: false,
        error: error.message || 'Failed to refresh user data',
      });
    }
  }, []);

  // Initialize authentication
  useEffect(() => {
    const initAuth = async () => {
      try {
        // Initialize Facebook SDK (non-blocking - don't fail if Facebook SDK fails)
        try {
          await initFacebookSDK();
          console.log('Facebook SDK initialized successfully');
        } catch (facebookError) {
          console.warn('Facebook SDK initialization failed:', facebookError);
          // Continue without Facebook SDK - it will be handled when user tries to use it
        }

        // Check for existing session
        const token = localStorage.getItem('auth_token');
        if (token) {
          await refreshUser();
        } else {
          setAuthState(prev => ({ ...prev, isLoading: false }));
        }
      } catch (error) {
        console.error('Auth initialization error:', error);
        setAuthState(prev => ({
          ...prev,
          isLoading: false,
          error: 'Failed to initialize authentication',
        }));
      }
    };

    initAuth();
  }, [refreshUser]); // Include refreshUser in dependency array

  const signInWithGoogle = useCallback(async () => {
    try {
      setAuthState(prev => ({ ...prev, isLoading: true, error: null }));

      // Check if Google Client ID is configured
      let result: any;
      if (!GOOGLE_CLIENT_ID || GOOGLE_CLIENT_ID === '') {
        // Demo mode when Google Client ID is not configured
        result = {
          id: 'google-demo-user-123',
          email: 'demo@econgraph.com',
          name: 'Demo Google User',
          imageUrl: 'https://via.placeholder.com/100',
          authentication: {
            idToken: 'demo-id-token',
            accessToken: 'demo-access-token',
          },
        };
      } else {
        // Real Google OAuth flow
        result = await new Promise((resolve, reject) => {
          // Load Google Identity Services
          if (typeof (window as any).google === 'undefined') {
            const script = document.createElement('script');
            script.src = 'https://accounts.google.com/gsi/client';
            script.onload = () => {
              (window as any).google.accounts.id.initialize({
                client_id: GOOGLE_CLIENT_ID,
                callback: (response: any) => {
                  try {
                    // Decode the JWT token to get user info
                    const payload = JSON.parse(atob(response.credential.split('.')[1]));
                    resolve({
                      id: payload.sub,
                      email: payload.email,
                      name: payload.name,
                      imageUrl: payload.picture,
                      authentication: {
                        idToken: response.credential,
                        accessToken: response.credential,
                      },
                    });
                  } catch (error) {
                    reject(error);
                  }
                },
              });
              (window as any).google.accounts.id.prompt();
            };
            script.onerror = reject;
            document.head.appendChild(script);
          } else {
            (window as any).google.accounts.id.initialize({
              client_id: GOOGLE_CLIENT_ID,
              callback: (response: any) => {
                try {
                  const payload = JSON.parse(atob(response.credential.split('.')[1]));
                  resolve({
                    id: payload.sub,
                    email: payload.email,
                    name: payload.name,
                    imageUrl: payload.picture,
                    authentication: {
                      idToken: response.credential,
                      accessToken: response.credential,
                    },
                  });
                } catch (error) {
                  reject(error);
                }
              },
            });
            (window as any).google.accounts.id.prompt();
          }
        });
      }

      if (result) {
        // Send Google token to backend for verification and user creation/login
        const response = await fetch(`${API_BASE_URL}/auth/google`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            token: result.authentication.idToken,
            user_info: {
              id: result.id,
              email: result.email,
              name: result.name,
              avatar: result.imageUrl,
            },
          }),
        });

        if (!response.ok) {
          throw new Error('Google authentication failed');
        }

        const authData = await response.json();

        // Store auth token
        localStorage.setItem('auth_token', authData.token);

        setAuthState({
          user: authData.user,
          isAuthenticated: true,
          isLoading: false,
          error: null,
        });
      }
    } catch (error: any) {
      setAuthState(prev => ({
        ...prev,
        isLoading: false,
        error: error.message || 'Google sign-in failed',
      }));
    }
  }, []);

  const signInWithFacebook = useCallback(async () => {
    try {
      setAuthState(prev => ({ ...prev, isLoading: true, error: null }));

      // Check if Facebook App ID is configured
      if (
        !FACEBOOK_APP_ID ||
        FACEBOOK_APP_ID.trim() === '' ||
        FACEBOOK_APP_ID === 'demo-facebook-app-id'
      ) {
        // Demo mode - simulate Facebook authentication
        console.log('Facebook authentication in demo mode');

        // Simulate a delay for realistic UX
        await new Promise(resolve => setTimeout(resolve, 1000));

        // Create demo Facebook user data
        const demoUserInfo = {
          id: 'demo-facebook-user-123',
          email: 'demo.facebook@econgraph.com',
          name: 'Demo Facebook User',
          picture: {
            data: {
              url: 'https://via.placeholder.com/100',
            },
          },
        };

        // Send demo Facebook data to backend
        const response = await fetch(`${API_BASE_URL}/auth/facebook`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            facebook_id: demoUserInfo.id,
            user_info: {
              id: demoUserInfo.id,
              email: demoUserInfo.email,
              name: demoUserInfo.name,
              avatar: demoUserInfo.picture?.data?.url,
            },
          }),
        });

        if (!response.ok) {
          const errorData = await response.json().catch(() => ({}));
          throw new Error(errorData.message || 'Facebook authentication failed');
        }

        const authData = await response.json();

        // Store auth token
        localStorage.setItem('auth_token', authData.token);

        setAuthState({
          user: authData.user,
          isAuthenticated: true,
          isLoading: false,
          error: null,
        });
        return;
      }

      // Check if Facebook SDK is available
      if (!window.FB) {
        throw new Error('Facebook SDK not available. Please refresh the page and try again.');
      }

      // Facebook login with timeout
      await new Promise<any>((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Facebook login timeout. Please try again.'));
        }, 30000); // 30 second timeout

        window.FB.login(
          (response: any) => {
            clearTimeout(timeout);
            if (response.authResponse) {
              resolve(response);
            } else {
              reject(new Error('Facebook login cancelled or failed'));
            }
          },
          { scope: 'email,public_profile' }
        );
      });

      // Get user info from Facebook with timeout
      const userInfo = await new Promise<any>((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error('Failed to get Facebook user info - timeout'));
        }, 10000); // 10 second timeout

        window.FB.api('/me', { fields: 'id,name,email,picture' }, (response: any) => {
          clearTimeout(timeout);
          if (response && !response.error) {
            resolve(response);
          } else {
            reject(new Error('Failed to get Facebook user info'));
          }
        });
      });

      // Send Facebook data to backend
      const response = await fetch(`${API_BASE_URL}/auth/facebook`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          facebook_id: userInfo.id,
          user_info: {
            id: userInfo.id,
            email: userInfo.email,
            name: userInfo.name,
            avatar: userInfo.picture?.data?.url,
          },
        }),
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new Error(errorData.message || 'Facebook authentication failed');
      }

      const authData = await response.json();

      // Store auth token
      localStorage.setItem('auth_token', authData.token);

      setAuthState({
        user: authData.user,
        isAuthenticated: true,
        isLoading: false,
        error: null,
      });
    } catch (error: any) {
      setAuthState(prev => ({
        ...prev,
        isLoading: false,
        error: error.message || 'Facebook sign-in failed',
      }));
    }
  }, []);

  const signInWithEmail = useCallback(async (email: string, password: string) => {
    try {
      setAuthState(prev => ({ ...prev, isLoading: true, error: null }));

      const response = await fetch(`${API_BASE_URL}/auth/login`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ email, password }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.message || 'Email sign-in failed');
      }

      const authData = await response.json();

      // Store auth token
      localStorage.setItem('auth_token', authData.token);

      setAuthState({
        user: authData.user,
        isAuthenticated: true,
        isLoading: false,
        error: null,
      });
    } catch (error: any) {
      setAuthState(prev => ({
        ...prev,
        isLoading: false,
        error: error.message || 'Email sign-in failed',
      }));
    }
  }, []);

  const signUp = useCallback(async (email: string, password: string, name: string) => {
    try {
      setAuthState(prev => ({ ...prev, isLoading: true, error: null }));

      const response = await fetch(`${API_BASE_URL}/auth/register`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ email, password, name }),
      });

      if (!response.ok) {
        const errorData = await response.json();
        throw new Error(errorData.message || 'Registration failed');
      }

      const authData = await response.json();

      // Store auth token
      localStorage.setItem('auth_token', authData.token);

      setAuthState({
        user: authData.user,
        isAuthenticated: true,
        isLoading: false,
        error: null,
      });
    } catch (error: any) {
      setAuthState(prev => ({
        ...prev,
        isLoading: false,
        error: error.message || 'Registration failed',
      }));
    }
  }, []);

  const signOut = useCallback(async () => {
    try {
      setAuthState(prev => ({ ...prev, isLoading: true }));

      const token = localStorage.getItem('auth_token');
      if (token) {
        // Notify backend of logout
        await fetch(`${API_BASE_URL}/auth/logout`, {
          method: 'POST',
          headers: {
            Authorization: `Bearer ${token}`,
          },
        });
      }

      // In production, this would sign out from Google OAuth
      console.log('Google sign out (demo mode)');

      // Sign out from Facebook
      try {
        window.FB?.logout();
      } catch (error) {
        console.log('Facebook sign out error (may not be signed in):', error);
      }

      // Clear local storage
      localStorage.removeItem('auth_token');

      setAuthState({
        user: null,
        isAuthenticated: false,
        isLoading: false,
        error: null,
      });
    } catch (error: any) {
      console.error('Sign out error:', error);
      // Force logout even if backend call fails
      localStorage.removeItem('auth_token');
      setAuthState({
        user: null,
        isAuthenticated: false,
        isLoading: false,
        error: null,
      });
    }
  }, []);

  const updateProfile = useCallback(async (updates: Partial<User>) => {
    try {
      const token = localStorage.getItem('auth_token');
      if (!token) throw new Error('No authentication token');

      const response = await fetch(`${API_BASE_URL}/auth/profile`, {
        method: 'PATCH',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify(updates),
      });

      if (!response.ok) {
        throw new Error('Profile update failed');
      }

      const updatedUser = await response.json();

      setAuthState(prev => ({
        ...prev,
        user: updatedUser.user,
      }));
    } catch (error: any) {
      setAuthState(prev => ({
        ...prev,
        error: error.message || 'Profile update failed',
      }));
    }
  }, []);

  // refreshUser function moved above to resolve dependency issue

  const clearError = useCallback(() => {
    setAuthState(prev => ({ ...prev, error: null }));
  }, []);

  const contextValue: AuthContextType = {
    ...authState,
    signInWithGoogle,
    signInWithFacebook,
    signInWithEmail,
    signUp,
    signOut,
    updateProfile,
    refreshUser,
    clearError,
  };

  return <AuthContext.Provider value={contextValue}>{children}</AuthContext.Provider>;
};

export const useAuth = (): AuthContextType => {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};

export default AuthContext;
