/**
 * REQUIREMENT: Test theme context functionality
 * PURPOSE: Verify theme switching and persistence works correctly
 * This ensures user preferences are properly managed and applied.
 */

import React from 'react';
import { render, screen, act, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import { ThemeProvider, useTheme } from '../ThemeContext';

// Mock the AuthContext
vi.mock('../AuthContext', async () => {
  const actual = await vi.importActual('../AuthContext');
  return {
    ...actual,
    useAuth: () => ({
      user: null, // No user in tests to avoid user preference interference
      isAuthenticated: false,
      login: vi.fn(),
      logout: vi.fn(),
    }),
    AuthProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  };
});

// localStorage mock is now handled globally in setupTests.vitest.ts

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
    vi.clearAllMocks();
  });

  it('should initialize with light theme by default', () => {
    // This test is covered by the next test
  });

  it('should initialize with light theme when localStorage is null', () => {
    vi.spyOn(window.localStorage, 'getItem').mockReturnValue(null);

    render(
      <TestWrapper>
        <TestComponent />
      </TestWrapper>
    );

    expect(screen.getByTestId('current-theme')).toHaveTextContent('light');
  });

  it('should load theme from localStorage', async () => {
    vi.spyOn(window.localStorage, 'getItem').mockReturnValue('dark');

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
    vi.spyOn(window.localStorage, 'getItem').mockReturnValue('light');
    vi.spyOn(window.localStorage, 'setItem').mockImplementation(() => {
      // Mock implementation
    });

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
    vi.spyOn(window.localStorage, 'getItem').mockReturnValue('dark');
    vi.spyOn(window.localStorage, 'setItem').mockImplementation(() => {
      // Mock implementation
    });

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
    vi.spyOn(window.localStorage, 'getItem').mockReturnValue('light');
    vi.spyOn(window.localStorage, 'setItem').mockImplementation(() => {
      // Mock implementation
    });

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
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {
      // Mock console.error implementation
    });

    expect(() => {
      render(<TestComponent />);
    }).toThrow('useTheme must be used within a ThemeProvider');

    consoleSpy.mockRestore();
  });
});
