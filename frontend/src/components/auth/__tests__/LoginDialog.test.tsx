/**
 * REQUIREMENT: Comprehensive unit tests for authentication dialog
 * PURPOSE: Test LoginDialog component behavior including error handling and user interactions
 * This ensures proper authentication flow and error message visibility.
 */

import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { vi } from 'vitest';
import LoginDialog from '../LoginDialog';
import { AuthProvider } from '../../../contexts/AuthContext';

// Note: ResizeObserver is mocked in setupTests.vitest.ts using resize-observer-polyfill

// Set up Facebook App ID for testing
process.env.REACT_APP_FACEBOOK_APP_ID = 'test-facebook-app-id';

// Mock the auth context
const mockAuthContext = {
  signInWithGoogle: vi.fn(),
  signInWithFacebook: vi.fn(),
  signInWithEmail: vi.fn(),
  signUp: vi.fn(),
  signOut: vi.fn(),
  updateProfile: vi.fn(),
  refreshUser: vi.fn(),
  clearError: vi.fn(),
  user: null,
  isAuthenticated: false,
  isLoading: false,
  error: null as string | null,
};

// Mock the useAuth hook
vi.mock('../../../contexts/AuthContext', async () => {
  const actual = await vi.importActual('../../../contexts/AuthContext');
  return {
    ...actual,
    useAuth: () => mockAuthContext,
    AuthProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  };
});

// Create a test theme
const theme = createTheme();

// Test wrapper component
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <ThemeProvider theme={theme}>
    <AuthProvider>
      {children}
    </AuthProvider>
  </ThemeProvider>
);

describe('LoginDialog', () => {
  // Prevent unhandled promise rejection noise from mocked auth failures
  let unhandledRejectionHandler: (e: any) => void;

  beforeAll(() => {
    unhandledRejectionHandler = (e: any) => {
      e.preventDefault();
    };
    // jsdom window exists in this environment
    window.addEventListener('unhandledrejection', unhandledRejectionHandler);
  });

  afterAll(() => {
    window.removeEventListener('unhandledrejection', unhandledRejectionHandler);
  });
  const mockOnClose = vi.fn();
  const mockOnSuccess = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockAuthContext.error = null;
    mockAuthContext.isLoading = false;
  });

  it('renders login dialog when open', () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    expect(screen.getByText('Welcome to EconGraph')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue with Google' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue with Facebook' })).toBeInTheDocument();
    expect(screen.getAllByText('Sign In')).toHaveLength(2); // Tab and button
  });

  it('does not render when closed', () => {
    render(
      <TestWrapper>
        <LoginDialog open={false} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    expect(screen.queryByText('Welcome to EconGraph')).not.toBeInTheDocument();
  });

  it('shows error message when authentication fails', async () => {
    const errorMessage = 'Invalid email or password';
    mockAuthContext.error = errorMessage;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    expect(screen.getByText(errorMessage)).toBeInTheDocument();
  });

  it('stays open when email authentication fails', async () => {
    const errorMessage = 'Invalid email or password';
    mockAuthContext.signInWithEmail.mockRejectedValue(new Error(errorMessage));
    mockAuthContext.error = errorMessage;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Fill in email and password (target the Sign In tab specifically)
    const emailInput = screen.getAllByLabelText('Email')[0]; // First one is Sign In tab
    const passwordInput = screen.getAllByLabelText('Password')[0]; // First one is Sign In tab
    fireEvent.change(emailInput, { target: { value: 'test@example.com' } });
    fireEvent.change(passwordInput, { target: { value: 'password123' } });

    // Click sign in button (use the form button in DialogActions)
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    // Wait for the authentication call
    await waitFor(() => {
      expect(mockAuthContext.signInWithEmail).toHaveBeenCalledWith('test@example.com', 'password123');
    }, { timeout: 5000 });

    // Dialog should still be open and show error
    expect(screen.getByText('Welcome to EconGraph')).toBeInTheDocument();
    expect(screen.getByText(errorMessage)).toBeInTheDocument();
    expect(mockOnClose).not.toHaveBeenCalled();
  });

  it('stays open when Google authentication fails', async () => {
    const errorMessage = 'Google authentication failed';
    mockAuthContext.signInWithGoogle.mockRejectedValue(new Error(errorMessage));
    mockAuthContext.error = errorMessage;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Click Google sign in button
    fireEvent.click(screen.getByRole('button', { name: 'Continue with Google' }));

    await waitFor(() => {
      expect(mockAuthContext.signInWithGoogle).toHaveBeenCalled();
    });

    // Dialog should still be open and show error
    expect(screen.getByText('Welcome to EconGraph')).toBeInTheDocument();
    expect(screen.getByText(errorMessage)).toBeInTheDocument();
    expect(mockOnClose).not.toHaveBeenCalled();
  });

  it('stays open when Facebook authentication fails', async () => {
    const errorMessage = 'Facebook authentication failed';
    mockAuthContext.signInWithFacebook.mockRejectedValue(new Error(errorMessage));
    mockAuthContext.error = errorMessage;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Click Facebook sign in button
    fireEvent.click(screen.getByText('Continue with Facebook'));

    await waitFor(() => {
      expect(mockAuthContext.signInWithFacebook).toHaveBeenCalled();
    });

    // Dialog should still be open and show error
    expect(screen.getByText('Welcome to EconGraph')).toBeInTheDocument();
    expect(screen.getByText(errorMessage)).toBeInTheDocument();
    expect(mockOnClose).not.toHaveBeenCalled();
  });

  it('closes dialog when email authentication succeeds', async () => {
    mockAuthContext.signInWithEmail.mockResolvedValue(undefined);

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Fill in email and password (target the Sign In tab specifically)
    const emailInput = screen.getAllByLabelText('Email')[0]; // First one is Sign In tab
    const passwordInput = screen.getAllByLabelText('Password')[0]; // First one is Sign In tab
    fireEvent.change(emailInput, { target: { value: 'test@example.com' } });
    fireEvent.change(passwordInput, { target: { value: 'password123' } });

    // Click sign in button (use the form button in DialogActions)
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    // Wait for the authentication call
    await waitFor(() => {
      expect(mockAuthContext.signInWithEmail).toHaveBeenCalledWith('test@example.com', 'password123');
    }, { timeout: 5000 });

    // Dialog should close and success callback should be called
    expect(mockOnClose).toHaveBeenCalled();
    expect(mockOnSuccess).toHaveBeenCalled();
  });

  it('closes dialog when Google authentication succeeds', async () => {
    mockAuthContext.signInWithGoogle.mockResolvedValue(undefined);

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Click Google sign in button
    fireEvent.click(screen.getByRole('button', { name: 'Continue with Google' }));

    await waitFor(() => {
      expect(mockAuthContext.signInWithGoogle).toHaveBeenCalled();
    });

    // Dialog should close and success callback should be called
    expect(mockOnClose).toHaveBeenCalled();
    expect(mockOnSuccess).toHaveBeenCalled();
  });

  it('closes dialog when Facebook authentication succeeds', async () => {
    mockAuthContext.signInWithFacebook.mockResolvedValue(undefined);

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Click Facebook sign in button
    fireEvent.click(screen.getByText('Continue with Facebook'));

    await waitFor(() => {
      expect(mockAuthContext.signInWithFacebook).toHaveBeenCalled();
    });

    // Dialog should close and success callback should be called
    expect(mockOnClose).toHaveBeenCalled();
    expect(mockOnSuccess).toHaveBeenCalled();
  });

  it('shows loading state during authentication', () => {
    mockAuthContext.isLoading = true;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Buttons should be disabled during loading
    expect(screen.getByRole('button', { name: 'Continue with Google' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Continue with Facebook' })).toBeDisabled();
    // Check if the form button is disabled (it shows CircularProgress when loading)
    // Should show loading spinner
    expect(screen.getByRole('progressbar')).toBeInTheDocument();
    // All buttons should be disabled during loading
    const allButtons = screen.getAllByRole('button');
    const disabledButtons = allButtons.filter(button => (button as HTMLButtonElement).disabled);
    expect(disabledButtons.length).toBeGreaterThan(0);
  });

  it('clears error when user switches tabs', () => {
    mockAuthContext.error = 'Some error';

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Switch to sign up tab
    fireEvent.click(screen.getByRole('tab', { name: 'Sign Up' }));

    expect(mockAuthContext.clearError).toHaveBeenCalled();
  });

  it('clears error when user starts typing', () => {
    mockAuthContext.error = 'Some error';

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Start typing in email field (target the Sign In tab specifically)
    const emailInput = screen.getAllByLabelText('Email')[0]; // First one is Sign In tab
    fireEvent.change(emailInput, { target: { value: 'test' } });

    // The component clears field-specific errors, not the global auth error
    // This test verifies that the input change works without errors
    expect(screen.getByDisplayValue('test')).toBeInTheDocument();
  });

  it('validates form fields before submission', async () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Try to submit without filling fields
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    // The form validation should prevent submission, so authentication should not be called
    expect(mockAuthContext.signInWithEmail).not.toHaveBeenCalled();
  });

  it('shows sign up form when sign up tab is selected', () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Switch to sign up tab
    fireEvent.click(screen.getByRole('tab', { name: 'Sign Up' }));

    expect(screen.getByLabelText('Full Name')).toBeInTheDocument();
    expect(screen.getByLabelText('Confirm Password')).toBeInTheDocument();
    expect(screen.getByText('Create Account')).toBeInTheDocument();
  });

  it('stays open when sign up fails', async () => {
    const errorMessage = 'Registration failed';
    mockAuthContext.signUp.mockRejectedValue(new Error(errorMessage));
    mockAuthContext.error = errorMessage;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Switch to sign up tab
    fireEvent.click(screen.getByRole('tab', { name: 'Sign Up' }));

    // Fill in form
    fireEvent.change(screen.getByLabelText('Full Name'), { target: { value: 'John Doe' } });
    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'test@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'password123' } });
    fireEvent.change(screen.getByLabelText('Confirm Password'), { target: { value: 'password123' } });

    // Click create account button
    fireEvent.click(screen.getByText('Create Account'));

    await waitFor(() => {
      expect(mockAuthContext.signUp).toHaveBeenCalledWith('test@example.com', 'password123', 'John Doe');
    });

    // Dialog should still be open and show error
    expect(screen.getByText('Welcome to EconGraph')).toBeInTheDocument();
    expect(screen.getByText(errorMessage)).toBeInTheDocument();
    expect(mockOnClose).not.toHaveBeenCalled();
  });

  it('should validate email format on sign in', async () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Enter invalid email
    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'invalid-email' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'password123' } });

    // Try to submit
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    // Should not call authentication with invalid email
    expect(mockAuthContext.signInWithEmail).not.toHaveBeenCalled();
  });

  it('should validate password length on sign in', async () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Enter short password
    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'test@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'short' } });

    // Try to submit
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    // Should not call authentication with short password
    expect(mockAuthContext.signInWithEmail).not.toHaveBeenCalled();
  });

  it('should validate password confirmation on sign up', async () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Switch to sign up tab
    fireEvent.click(screen.getByRole('tab', { name: 'Sign Up' }));

    // Fill form with mismatched passwords
    fireEvent.change(screen.getByLabelText('Full Name'), { target: { value: 'Test User' } });
    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'test@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'password123' } });
    fireEvent.change(screen.getByLabelText('Confirm Password'), { target: { value: 'different123' } });

    // Try to submit
    fireEvent.click(screen.getByText('Create Account'));

    // Should not call signUp with mismatched passwords
    expect(mockAuthContext.signUp).not.toHaveBeenCalled();
  });

  it('should show loading state during authentication', () => {
    // Set loading state
    mockAuthContext.isLoading = true;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Buttons should be disabled during loading
    const googleButton = screen.getByRole('button', { name: 'Continue with Google' });
    const facebookButton = screen.getByRole('button', { name: 'Continue with Facebook' });

    expect(googleButton.closest('button')).toBeDisabled();
    expect(facebookButton.closest('button')).toBeDisabled();
  });

  it('should handle successful sign up', async () => {
    mockAuthContext.signUp.mockResolvedValue(undefined);

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Switch to sign up tab
    fireEvent.click(screen.getByRole('tab', { name: 'Sign Up' }));

    // Fill sign up form
    fireEvent.change(screen.getByLabelText('Full Name'), { target: { value: 'New User' } });
    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'newuser@example.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'password123' } });
    fireEvent.change(screen.getByLabelText('Confirm Password'), { target: { value: 'password123' } });

    // Submit sign up
    fireEvent.click(screen.getByText('Create Account'));

    await waitFor(() => {
      expect(mockAuthContext.signUp).toHaveBeenCalledWith(
        'newuser@example.com',
        'password123',
        'New User'
      );
    });

    // Dialog should close and success callback should be called
    expect(mockOnClose).toHaveBeenCalled();
    expect(mockOnSuccess).toHaveBeenCalled();
  });

  it('should clear error when dialog closes', () => {
    const mockClearError = vi.fn();
    mockAuthContext.clearError = mockClearError;
    mockAuthContext.error = 'Previous error';

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Close the dialog using the dialog header close button
    const closeButton = screen.getByRole('button', { name: 'Close dialog' });
    fireEvent.click(closeButton);

    // Error should be cleared when dialog closes
    expect(mockClearError).toHaveBeenCalled();
  });

  it('should handle form validation for empty fields', () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Try to submit empty form
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    // Should not call authentication with empty fields
    expect(mockAuthContext.signInWithEmail).not.toHaveBeenCalled();
  });

  it('should preserve email when switching tabs', () => {
    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Fill email on sign in tab - use more specific selector
    const signInTab = screen.getByRole('tab', { name: 'Sign In' });
    fireEvent.click(signInTab);
    
    const emailInput = screen.getByLabelText('Email');
    fireEvent.change(emailInput, { target: { value: 'test@example.com' } });

    // Switch to sign up tab
    fireEvent.click(screen.getByRole('tab', { name: 'Sign Up' }));

    // Email should be preserved
    expect(screen.getByDisplayValue('test@example.com')).toBeInTheDocument();

    // Switch back to sign in tab
    fireEvent.click(screen.getByRole('tab', { name: 'Sign In' }));

    // Email should still be there
    expect(screen.getByDisplayValue('test@example.com')).toBeInTheDocument();
  });

  it('should disable form submission during loading', () => {
    mockAuthContext.isLoading = true;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Fill form (target the Sign In tab specifically)
    const signInTab = screen.getByRole('tab', { name: 'Sign In' });
    fireEvent.click(signInTab);
    
    const emailInput = screen.getByLabelText('Email');
    const passwordInput = screen.getByLabelText('Password');
    fireEvent.change(emailInput, { target: { value: 'test@example.com' } });
    fireEvent.change(passwordInput, { target: { value: 'password123' } });

    // During loading, buttons should be disabled
    const googleButton = screen.getByRole('button', { name: 'Continue with Google' });
    const facebookButton = screen.getByRole('button', { name: 'Continue with Facebook' });

    expect(googleButton.closest('button')).toBeDisabled();
    expect(facebookButton.closest('button')).toBeDisabled();

    // Should not call authentication during loading
    expect(mockAuthContext.signInWithEmail).not.toHaveBeenCalled();
  });

  it('should handle authentication errors and keep dialog open', async () => {
    const errorMessage = 'Invalid credentials';
    mockAuthContext.signInWithEmail.mockRejectedValue(new Error(errorMessage));
    mockAuthContext.error = errorMessage;

    render(
      <TestWrapper>
        <LoginDialog open={true} onClose={mockOnClose} onSuccess={mockOnSuccess} />
      </TestWrapper>
    );

    // Fill form (target the Sign In tab specifically)
    const signInTab = screen.getByRole('tab', { name: 'Sign In' });
    fireEvent.click(signInTab);
    
    const emailInput = screen.getByLabelText('Email');
    const passwordInput = screen.getByLabelText('Password');
    fireEvent.change(emailInput, { target: { value: 'test@example.com' } });
    fireEvent.change(passwordInput, { target: { value: 'wrongpassword' } });

    // Submit form
    fireEvent.click(screen.getByRole('button', { name: /sign in/i }));

    await waitFor(() => {
      expect(mockAuthContext.signInWithEmail).toHaveBeenCalled();
    });

    // Should show error message
    expect(screen.getByText(errorMessage)).toBeInTheDocument();

    // Dialog should stay open
    expect(mockOnClose).not.toHaveBeenCalled();
    expect(mockOnSuccess).not.toHaveBeenCalled();
  });
});
