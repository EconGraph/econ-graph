/**
 * REQUIREMENT: Test user profile preferences functionality
 * PURPOSE: Verify user preferences can be edited and saved correctly
 * This ensures the preferences UI works as expected
 */

import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import UserProfile from '../UserProfile';
import { AuthProvider } from '../../../contexts/AuthContext';
import { ThemeProvider } from '../../../contexts/ThemeContext';

// Mock user data
const mockUser = {
  id: '1',
  email: 'test@example.com',
  name: 'Test User',
  avatar: 'https://example.com/avatar.jpg',
  provider: 'google' as const,
  role: 'analyst' as const,
  organization: 'Test Org',
  preferences: {
    theme: 'light' as const,
    defaultChartType: 'line',
    notifications: true,
    collaborationEnabled: true,
  },
  createdAt: '2023-01-01T00:00:00Z',
  lastLoginAt: '2023-01-01T00:00:00Z',
};

// Mock fetch for API calls
global.fetch = jest.fn();

// localStorage mock is now handled globally in setupTests.ts

// Mock fetch to return user data
(fetch as jest.Mock).mockImplementation((url) => {
  if (url.includes('/auth/me')) {
    return Promise.resolve({
      ok: true,
      json: () => Promise.resolve({ user: mockUser }),
    });
  }
  if (url.includes('/auth/profile')) {
    return Promise.resolve({
      ok: true,
      json: () => Promise.resolve({ user: mockUser }),
    });
  }
  return Promise.resolve({
    ok: true,
    json: () => Promise.resolve({}),
  });
});

// Create a mock useAuth hook that can be controlled per test
const mockUseAuth = jest.fn();

// Mock the AuthContext module
jest.mock('../../../contexts/AuthContext', () => ({
  ...jest.requireActual('../../../contexts/AuthContext'),
  useAuth: () => mockUseAuth(),
  AuthProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

// Mock the useTheme hook
jest.mock('../../../contexts/ThemeContext', () => ({
  ...jest.requireActual('../../../contexts/ThemeContext'),
  useTheme: () => ({
    theme: {} as any,
    toggleTheme: jest.fn(),
    setTheme: jest.fn(),
    currentTheme: 'light' as const,
  }),
}));

// Wrapper component that provides all necessary contexts
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <AuthProvider>
    <ThemeProvider>
      {children}
    </ThemeProvider>
  </AuthProvider>
);

describe('UserProfile Preferences', () => {
  beforeEach(() => {
    // Reset all mocks to prevent test pollution
    jest.clearAllMocks();

    // Setup localStorage mock for this test
    (window.localStorage.getItem as jest.Mock).mockImplementation((key) => {
      if (key === 'auth_token') return 'mock-token';
      if (key === 'theme') return 'light';
      return null;
    });

    // Setup default AuthContext mock for each test
    mockUseAuth.mockReturnValue({
      user: mockUser,
      isAuthenticated: true,
      isLoading: false,
      error: null,
      signInWithGoogle: jest.fn(),
      signInWithFacebook: jest.fn(),
      signInWithEmail: jest.fn(),
      signUp: jest.fn(),
      signOut: jest.fn(),
      updateProfile: jest.fn().mockResolvedValue({}),
      refreshUser: jest.fn(),
      clearError: jest.fn(),
    });
  });


  it('should render preferences section with current values', async () => {
    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    // Wait for the component to load
    await waitFor(() => {
      expect(screen.getByText('Preferences')).toBeInTheDocument();
    });

    expect(screen.getByDisplayValue('light')).toBeInTheDocument();
    expect(screen.getByDisplayValue('line')).toBeInTheDocument();
  });

  it('should allow theme selection without edit mode', async () => {
    const user = userEvent.setup();

    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    // Find the theme select by its label (now properly associated)
    const themeSelect = screen.getByLabelText('Theme');
    expect(themeSelect).not.toBeDisabled();

    // Use fireEvent instead of userEvent for Material-UI Select
    fireEvent.mouseDown(themeSelect);
    await user.click(screen.getByText('Dark'));

    expect(screen.getByDisplayValue('dark')).toBeInTheDocument();
  });

  it('should allow default chart type selection without edit mode', async () => {
    const user = userEvent.setup();

    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    // Find the chart type select by its label (now properly associated)
    const chartTypeSelect = screen.getByLabelText('Default Chart Type');
    expect(chartTypeSelect).not.toBeDisabled();

    // Use fireEvent instead of userEvent for Material-UI Select
    fireEvent.mouseDown(chartTypeSelect);
    await user.click(screen.getByText('Area Chart'));

    expect(screen.getByDisplayValue('area')).toBeInTheDocument();
  });

  it('should allow notification toggle without edit mode', async () => {
    const user = userEvent.setup();

    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    const notificationSwitch = screen.getByRole('checkbox', { name: /email notifications/i });
    expect(notificationSwitch).not.toBeDisabled();
    expect(notificationSwitch).toBeChecked();

    await user.click(notificationSwitch);
    expect(notificationSwitch).not.toBeChecked();
  });

  it('should allow collaboration toggle without edit mode', async () => {
    const user = userEvent.setup();

    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    const collaborationSwitch = screen.getByRole('checkbox', { name: /enable chart collaboration/i });
    expect(collaborationSwitch).not.toBeDisabled();
    expect(collaborationSwitch).toBeChecked();

    await user.click(collaborationSwitch);
    expect(collaborationSwitch).not.toBeChecked();
  });

  it('should save preferences when save button is clicked', async () => {
    const user = userEvent.setup();
    const mockUpdateProfile = jest.fn().mockResolvedValue({});

    // Override the mock for this specific test
    mockUseAuth.mockReturnValue({
      user: mockUser,
      isAuthenticated: true,
      isLoading: false,
      error: null,
      signInWithGoogle: jest.fn(),
      signInWithFacebook: jest.fn(),
      signInWithEmail: jest.fn(),
      signUp: jest.fn(),
      signOut: jest.fn(),
      updateProfile: mockUpdateProfile,
      refreshUser: jest.fn(),
      clearError: jest.fn(),
    });

    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    const saveButton = screen.getByText('Save Preferences');
    expect(saveButton).toBeInTheDocument();

    await user.click(saveButton);

    // Verify that updateProfile was called
    expect(mockUpdateProfile).toHaveBeenCalled();
  });

  it('should show all available theme options', async () => {
    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    const themeSelect = screen.getByLabelText('Theme');
    fireEvent.mouseDown(themeSelect);

    // Check that both options are available in the dropdown
    expect(screen.getAllByText('Light')).toHaveLength(2); // One in input, one in dropdown
    expect(screen.getByText('Dark')).toBeInTheDocument();
  });

  it('should show all available chart type options', async () => {
    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    const chartTypeSelect = screen.getByLabelText('Default Chart Type');
    fireEvent.mouseDown(chartTypeSelect);

    // Check that all options are available in the dropdown
    expect(screen.getAllByText('Line Chart')).toHaveLength(2); // One in input, one in dropdown
    expect(screen.getByText('Area Chart')).toBeInTheDocument();
    expect(screen.getByText('Bar Chart')).toBeInTheDocument();
    expect(screen.getByText('Candlestick')).toBeInTheDocument();
  });

  it('should have save preferences button visible', () => {
    render(
      <TestWrapper>
        <UserProfile open={true} onClose={jest.fn()} />
      </TestWrapper>
    );

    const saveButton = screen.getByText('Save Preferences');
    expect(saveButton).toBeInTheDocument();
    expect(saveButton).toBeEnabled();
  });
});
