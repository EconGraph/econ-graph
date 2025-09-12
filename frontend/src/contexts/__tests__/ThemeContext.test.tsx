/**
 * REQUIREMENT: Test theme context functionality
 * PURPOSE: Verify theme switching and persistence works correctly
 * This ensures user preferences are properly managed and applied
 */

import React from 'react';
import { render, screen, act, waitFor } from '@testing-library/react';
import { ThemeProvider, useTheme } from '../ThemeContext';

// Mock the AuthContext
jest.mock('../AuthContext', () => ({
  useAuth: () => ({
    user: null, // No user in tests to avoid user preference interference
    isAuthenticated: false,
    login: jest.fn(),
    logout: jest.fn(),
  }),
  AuthProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// localStorage mock is now handled globally in setupTests.ts

// Test component that uses the theme context
const TestComponent: React.FC = () => {
  const { currentTheme, toggleTheme, setTheme } = useTheme();

  return (
    <div>
      <div data-testid="current-theme">{currentTheme}</div>
      <button data-testid="toggle-theme" onClick={toggleTheme}>
        Toggle Theme
      </button>
      <button data-testid="set-light" onClick={() => setTheme('light')}>
        Set Light
      </button>
      <button data-testid="set-dark" onClick={() => setTheme('dark')}>
        Set Dark
      </button>
    </div>
  );
};

// Wrapper component that provides theme context
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <ThemeProvider>
    {children}
  </ThemeProvider>
);

describe('ThemeContext', () => {
  beforeEach(() => {
    // Reset localStorage mock for each test
    (window.localStorage.getItem as jest.Mock).mockClear();
    (window.localStorage.setItem as jest.Mock).mockClear();
    (window.localStorage.removeItem as jest.Mock).mockClear();
    (window.localStorage.clear as jest.Mock).mockClear();
  });

  it('should initialize with light theme by default', () => {
    (window.localStorage.getItem as jest.Mock).mockReturnValue(null);
    jest.clearAllMocks();

    // Clear any existing localStorage state
    if (window.localStorage.clear) {
      window.localStorage.clear();
    }
  });

  it('should initialize with light theme when localStorage is null', () => {
    (window.localStorage.getItem as jest.Mock).mockReturnValue(null);

    render(
      <TestWrapper>
        <TestComponent />
      </TestWrapper>
    );

    expect(screen.getByTestId('current-theme')).toHaveTextContent('light');
  });

  it('should load theme from localStorage', async () => {
    (window.localStorage.getItem as jest.Mock).mockReturnValue('dark');

    render(
      <TestWrapper>
        <TestComponent />
      </TestWrapper>
    );

    // Wait for the theme to be loaded from localStorage
    await waitFor(() => {
      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });
  });

  it('should toggle theme correctly', async () => {
    (window.localStorage.getItem as jest.Mock).mockReturnValue('light');

    render(
      <TestWrapper>
        <TestComponent />
      </TestWrapper>
    );

    // Wait for initial theme to load
    await waitFor(() => {
      expect(screen.getByTestId('current-theme')).toHaveTextContent('light');
    });

    act(() => {
      screen.getByTestId('toggle-theme').click();
    });

    await waitFor(() => {
      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });

    expect(window.localStorage.setItem).toHaveBeenCalledWith('theme', 'dark');
  });

  it('should set theme to light', async () => {
    (window.localStorage.getItem as jest.Mock).mockReturnValue('dark');

    render(
      <TestWrapper>
        <TestComponent />
      </TestWrapper>
    );

    // Wait for initial theme to load from localStorage
    await waitFor(() => {
      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });

    act(() => {
      screen.getByTestId('set-light').click();
    });

    await waitFor(() => {
      expect(screen.getByTestId('current-theme')).toHaveTextContent('light');
    });

    expect(window.localStorage.setItem).toHaveBeenCalledWith('theme', 'light');
  });

  it('should set theme to dark', async () => {
    (window.localStorage.getItem as jest.Mock).mockReturnValue('light');

    render(
      <TestWrapper>
        <TestComponent />
      </TestWrapper>
    );

    // Wait for initial theme to load from localStorage
    await waitFor(() => {
      expect(screen.getByTestId('current-theme')).toHaveTextContent('light');
    });

    act(() => {
      screen.getByTestId('set-dark').click();
    });

    await waitFor(() => {
      expect(screen.getByTestId('current-theme')).toHaveTextContent('dark');
    });

    expect(window.localStorage.setItem).toHaveBeenCalledWith('theme', 'dark');
  });

  it('should throw error when used outside ThemeProvider', () => {
    // Suppress console.error for this test
    const consoleSpy = jest.spyOn(console, 'error').mockImplementation(() => {});

    expect(() => {
      render(<TestComponent />);
    }).toThrow('useTheme must be used within a ThemeProvider');

    consoleSpy.mockRestore();
  });
});
