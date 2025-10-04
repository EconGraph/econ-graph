/**
 * Error Handling Utilities.
 *
 * Utility functions and components for comprehensive error handling
 * and loading states in the Global Analysis components.
 */

import React from 'react';
import {
  Alert,
  AlertTitle,
  Box,
  Button,
  CircularProgress,
  Typography,
  Snackbar,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Stack,
} from '@mui/material';
import { ErrorOutline, Refresh, Warning, Info, CheckCircle } from '@mui/icons-material';

/**
 * Error types for different scenarios
 */
export enum ErrorType {
  NETWORK_ERROR = 'NETWORK_ERROR',
  DATA_LOADING_ERROR = 'DATA_LOADING_ERROR',
  RENDERING_ERROR = 'RENDERING_ERROR',
  VALIDATION_ERROR = 'VALIDATION_ERROR',
  PERMISSION_ERROR = 'PERMISSION_ERROR',
  UNKNOWN_ERROR = 'UNKNOWN_ERROR',
}

/**
 * Error severity levels
 */
export enum ErrorSeverity {
  LOW = 'low',
  MEDIUM = 'medium',
  HIGH = 'high',
  CRITICAL = 'critical',
}

/**
 * Error context interface
 */
export interface ErrorContext {
  type: ErrorType;
  severity: ErrorSeverity;
  message: string;
  details?: string;
  timestamp: Date;
  component?: string;
  action?: string;
  retryable: boolean;
}

/**
 * Create error context from error object
 */
export const createErrorContext = (
  error: Error | unknown,
  type: ErrorType = ErrorType.UNKNOWN_ERROR,
  component?: string,
  action?: string
): ErrorContext => {
  const message = error instanceof Error ? error.message : 'An unknown error occurred';
  const details = error instanceof Error ? error.stack : undefined;

  return {
    type,
    severity: getSeverityFromType(type),
    message,
    details,
    timestamp: new Date(),
    component,
    action,
    retryable: isRetryableError(type),
  };
};

/**
 * Get severity level from error type
 */
export const getSeverityFromType = (type: ErrorType): ErrorSeverity => {
  switch (type) {
    case ErrorType.NETWORK_ERROR:
    case ErrorType.DATA_LOADING_ERROR:
      return ErrorSeverity.HIGH;
    case ErrorType.RENDERING_ERROR:
      return ErrorSeverity.MEDIUM;
    case ErrorType.VALIDATION_ERROR:
      return ErrorSeverity.LOW;
    case ErrorType.PERMISSION_ERROR:
      return ErrorSeverity.CRITICAL;
    default:
      return ErrorSeverity.MEDIUM;
  }
};

/**
 * Check if error is retryable
 */
export const isRetryableError = (type: ErrorType): boolean => {
  switch (type) {
    case ErrorType.NETWORK_ERROR:
    case ErrorType.DATA_LOADING_ERROR:
      return true;
    case ErrorType.RENDERING_ERROR:
    case ErrorType.VALIDATION_ERROR:
    case ErrorType.PERMISSION_ERROR:
      return false;
    default:
      return true;
  }
};

/**
 * Get user-friendly error message
 */
export const getUserFriendlyMessage = (errorContext: ErrorContext): string => {
  switch (errorContext.type) {
    case ErrorType.NETWORK_ERROR:
      return 'Unable to connect to the server. Please check your internet connection and try again.';
    case ErrorType.DATA_LOADING_ERROR:
      return 'Failed to load data. The data source may be temporarily unavailable.';
    case ErrorType.RENDERING_ERROR:
      return 'There was a problem displaying the map. Please try refreshing the page.';
    case ErrorType.VALIDATION_ERROR:
      return 'The data format is invalid. Please contact support if this persists.';
    case ErrorType.PERMISSION_ERROR:
      return 'You do not have permission to access this data. Please contact your administrator.';
    default:
      return 'An unexpected error occurred. Please try again or contact support.';
  }
};

/**
 * Get error icon based on severity
 */
export const getErrorIcon = (severity: ErrorSeverity) => {
  switch (severity) {
    case ErrorSeverity.LOW:
      return <Info color='info' />;
    case ErrorSeverity.MEDIUM:
      return <Warning color='warning' />;
    case ErrorSeverity.HIGH:
      return <ErrorOutline color='error' />;
    case ErrorSeverity.CRITICAL:
      return <ErrorOutline color='error' />;
    default:
      return <ErrorOutline color='error' />;
  }
};

/**
 * Error boundary component for catching React errors
 */
export class ErrorBoundary extends React.Component<
  { children: React.ReactNode; fallback?: React.ReactNode },
  { hasError: boolean; error?: ErrorContext }
> {
  constructor(props: { children: React.ReactNode; fallback?: React.ReactNode }) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): { hasError: boolean; error: ErrorContext } {
    return {
      hasError: true,
      error: createErrorContext(error, ErrorType.RENDERING_ERROR, 'ErrorBoundary'),
    };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        this.props.fallback || (
          <ErrorDisplay
            error={this.state.error!}
            onRetry={() => this.setState({ hasError: false, error: undefined })}
          />
        )
      );
    }

    return this.props.children;
  }
}

/**
 * Error display component
 */
export const ErrorDisplay: React.FC<{
  error: ErrorContext;
  onRetry?: () => void;
  onDismiss?: () => void;
  showDetails?: boolean;
}> = ({ error, onRetry, onDismiss, showDetails = false }) => {
  const [showFullDetails, setShowFullDetails] = React.useState(false);

  return (
    <Alert
      severity={error.severity === ErrorSeverity.CRITICAL ? 'error' : 'warning'}
      icon={getErrorIcon(error.severity)}
      action={
        <Stack direction='row' spacing={1}>
          {error.retryable && onRetry && (
            <Button size='small' onClick={onRetry} startIcon={<Refresh />} variant='outlined'>
              Retry
            </Button>
          )}
          {onDismiss && (
            <Button size='small' onClick={onDismiss} variant='outlined'>
              Dismiss
            </Button>
          )}
        </Stack>
      }
    >
      <AlertTitle>{getUserFriendlyMessage(error)}</AlertTitle>
      {showDetails && (
        <Box sx={{ mt: 1 }}>
          <Button size='small' onClick={() => setShowFullDetails(!showFullDetails)}>
            {showFullDetails ? 'Hide' : 'Show'} Details
          </Button>
          {showFullDetails && (
            <Box sx={{ mt: 1, p: 1, bgcolor: 'grey.100', borderRadius: 1 }}>
              <Typography variant='caption' display='block'>
                <strong>Error Type:</strong> {error.type}
              </Typography>
              <Typography variant='caption' display='block'>
                <strong>Component:</strong> {error.component || 'Unknown'}
              </Typography>
              <Typography variant='caption' display='block'>
                <strong>Action:</strong> {error.action || 'Unknown'}
              </Typography>
              <Typography variant='caption' display='block'>
                <strong>Timestamp:</strong> {error.timestamp.toLocaleString()}
              </Typography>
              {error.details && (
                <Typography variant='caption' display='block'>
                  <strong>Details:</strong> {error.details}
                </Typography>
              )}
            </Box>
          )}
        </Box>
      )}
    </Alert>
  );
};

/**
 * Loading state component
 */
export const LoadingState: React.FC<{
  message?: string;
  progress?: number;
  size?: number;
}> = ({ message = 'Loading...', progress, size = 40 }) => {
  return (
    <Box
      sx={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        p: 3,
        gap: 2,
      }}
    >
      {progress !== undefined ? (
        <Box sx={{ position: 'relative', display: 'inline-flex' }}>
          <CircularProgress variant='determinate' value={progress} size={size} thickness={4} />
          <Box
            sx={{
              top: 0,
              left: 0,
              bottom: 0,
              right: 0,
              position: 'absolute',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <Typography variant='caption' component='div' color='text.secondary'>
              {`${Math.round(progress)}%`}
            </Typography>
          </Box>
        </Box>
      ) : (
        <CircularProgress size={size} />
      )}
      <Typography variant='body2' color='text.secondary'>
        {message}
      </Typography>
    </Box>
  );
};

/**
 * Empty state component
 */
export const EmptyState: React.FC<{
  title: string;
  message: string;
  action?: {
    label: string;
    onClick: () => void;
  };
  icon?: React.ReactNode;
}> = ({ title, message, action, icon }) => {
  return (
    <Box
      sx={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        p: 4,
        gap: 2,
        textAlign: 'center',
      }}
    >
      {icon || <Info color='disabled' sx={{ fontSize: 48 }} />}
      <Typography variant='h6' color='text.secondary'>
        {title}
      </Typography>
      <Typography variant='body2' color='text.secondary'>
        {message}
      </Typography>
      {action && (
        <Button variant='outlined' onClick={action.onClick} startIcon={<Refresh />}>
          {action.label}
        </Button>
      )}
    </Box>
  );
};

/**
 * Success state component
 */
export const SuccessState: React.FC<{
  message: string;
  onDismiss?: () => void;
}> = ({ message, onDismiss }) => {
  return (
    <Alert
      severity='success'
      icon={<CheckCircle />}
      action={
        onDismiss && (
          <Button size='small' onClick={onDismiss}>
            Dismiss
          </Button>
        )
      }
    >
      {message}
    </Alert>
  );
};

/**
 * Error dialog component
 */
export const ErrorDialog: React.FC<{
  open: boolean;
  error: ErrorContext;
  onClose: () => void;
  onRetry?: () => void;
}> = ({ open, error, onClose, onRetry }) => {
  return (
    <Dialog open={open} onClose={onClose} maxWidth='sm' fullWidth>
      <DialogTitle>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          {getErrorIcon(error.severity)}
          Error Details
        </Box>
      </DialogTitle>
      <DialogContent>
        <Typography variant='body1' gutterBottom>
          {getUserFriendlyMessage(error)}
        </Typography>
        <Box sx={{ mt: 2, p: 2, bgcolor: 'grey.100', borderRadius: 1 }}>
          <Typography variant='caption' display='block'>
            <strong>Error Type:</strong> {error.type}
          </Typography>
          <Typography variant='caption' display='block'>
            <strong>Severity:</strong> {error.severity}
          </Typography>
          <Typography variant='caption' display='block'>
            <strong>Component:</strong> {error.component || 'Unknown'}
          </Typography>
          <Typography variant='caption' display='block'>
            <strong>Timestamp:</strong> {error.timestamp.toLocaleString()}
          </Typography>
          {error.details && (
            <Typography variant='caption' display='block'>
              <strong>Technical Details:</strong>
            </Typography>
          )}
          {error.details && (
            <Typography
              variant='caption'
              component='pre'
              sx={{
                mt: 1,
                p: 1,
                bgcolor: 'white',
                borderRadius: 0.5,
                overflow: 'auto',
                maxHeight: 200,
              }}
            >
              {error.details}
            </Typography>
          )}
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Close</Button>
        {error.retryable && onRetry && (
          <Button onClick={onRetry} variant='contained' startIcon={<Refresh />}>
            Retry
          </Button>
        )}
      </DialogActions>
    </Dialog>
  );
};

/**
 * Error snackbar component
 */
export const ErrorSnackbar: React.FC<{
  open: boolean;
  error: ErrorContext;
  onClose: () => void;
  onRetry?: () => void;
}> = ({ open, error, onClose, onRetry }) => {
  return (
    <Snackbar
      open={open}
      autoHideDuration={error.severity === ErrorSeverity.CRITICAL ? 0 : 6000}
      onClose={onClose}
      anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
    >
      <Alert
        onClose={onClose}
        severity={error.severity === ErrorSeverity.CRITICAL ? 'error' : 'warning'}
        icon={getErrorIcon(error.severity)}
        action={
          error.retryable &&
          onRetry && (
            <Button size='small' onClick={onRetry} startIcon={<Refresh />} variant='outlined'>
              Retry
            </Button>
          )
        }
      >
        {getUserFriendlyMessage(error)}
      </Alert>
    </Snackbar>
  );
};

/**
 * Error handling hook
 */
export const useErrorHandler = () => {
  const [errors, setErrors] = React.useState<ErrorContext[]>([]);
  const [currentError, setCurrentError] = React.useState<ErrorContext | null>(null);

  const addError = React.useCallback((error: ErrorContext) => {
    setErrors(prev => [...prev, error]);
    setCurrentError(error);
  }, []);

  const removeError = React.useCallback((index: number) => {
    setErrors(prev => prev.filter((_, i) => i !== index));
  }, []);

  const clearErrors = React.useCallback(() => {
    setErrors([]);
    setCurrentError(null);
  }, []);

  const handleError = React.useCallback(
    (
      error: Error | unknown,
      type: ErrorType = ErrorType.UNKNOWN_ERROR,
      component?: string,
      action?: string
    ) => {
      const errorContext = createErrorContext(error, type, component, action);
      addError(errorContext);
    },
    [addError]
  );

  return {
    errors,
    currentError,
    addError,
    removeError,
    clearErrors,
    handleError,
  };
};
