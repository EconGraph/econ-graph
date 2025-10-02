/**
 * REQUIREMENT: OAuth login dialog with Google and Facebook authentication
 * PURPOSE: Provide secure login interface for chart collaboration features
 * This enables professional multi-user collaboration with proper authentication.
 */

import React, { useState, useCallback, useEffect } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  TextField,
  Box,
  Typography,
  Divider,
  Alert,
  CircularProgress,
  IconButton,
  Tab,
  Tabs,
  InputAdornment,
} from '@mui/material';
import {
  Google as GoogleIcon,
  Facebook as FacebookIcon,
  Visibility as VisibilityIcon,
  VisibilityOff as VisibilityOffIcon,
  Email as EmailIcon,
  Person as PersonIcon,
  Lock as LockIcon,
  Close as CloseIcon,
} from '@mui/icons-material';
import { useAuth } from '../../contexts/AuthContext';

interface LoginDialogProps {
  open: boolean;
  onClose: () => void;
  onSuccess?: () => void;
}

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

const TabPanel: React.FC<TabPanelProps> = ({ children, value, index, ...other }) => {
  return (
    <div
      role='tabpanel'
      hidden={value !== index}
      id={`auth-tabpanel-${index}`}
      aria-labelledby={`auth-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ pt: 2 }}>{children}</Box>}
    </div>
  );
};

const LoginDialog: React.FC<LoginDialogProps> = ({ open, onClose, onSuccess }) => {
  const {
    signInWithGoogle,
    signInWithFacebook,
    signInWithEmail,
    signUp,
    isLoading,
    error,
    clearError,
  } = useAuth();

  const [tabValue, setTabValue] = useState(0);
  const [showPassword, setShowPassword] = useState(false);
  const [formData, setFormData] = useState({
    email: '',
    password: '',
    name: '',
    confirmPassword: '',
  });
  const [formErrors, setFormErrors] = useState<{ [key: string]: string }>({});
  const [isFacebookAvailable, setIsFacebookAvailable] = useState(false);

  // Check if Facebook SDK is available
  useEffect(() => {
    const checkFacebookSDK = () => {
      // Check if Facebook App ID is configured (including demo mode)
      const facebookAppId = process.env.REACT_APP_FACEBOOK_APP_ID;
      const hasAppId = !!(facebookAppId && facebookAppId.trim() !== '');

      // Facebook is available if we have an App ID (including demo mode)
      setIsFacebookAvailable(hasAppId);
    };

    // Check immediately
    checkFacebookSDK();

    // Check again after a short delay in case SDK is still loading
    const timeout = setTimeout(checkFacebookSDK, 1000);

    return () => clearTimeout(timeout);
  }, []);

  const handleTabChange = useCallback(
    (event: React.SyntheticEvent, newValue: number) => {
      setTabValue(newValue);
      setFormErrors({});
      clearError();
    },
    [clearError]
  );

  const handleInputChange = useCallback(
    (field: string) => (event: React.ChangeEvent<HTMLInputElement>) => {
      setFormData(prev => ({ ...prev, [field]: event.target.value }));
      // Clear field error when user starts typing
      if (formErrors[field]) {
        setFormErrors(prev => ({ ...prev, [field]: '' }));
      }
    },
    [formErrors]
  );

  const validateForm = useCallback(() => {
    const errors: { [key: string]: string } = {};

    if (!formData.email) {
      errors.email = 'Email is required';
    } else if (!/\S+@\S+\.\S+/.test(formData.email)) {
      errors.email = 'Email is invalid';
    }

    if (!formData.password) {
      errors.password = 'Password is required';
    } else if (formData.password.length < 8) {
      errors.password = 'Password must be at least 8 characters';
    }

    if (tabValue === 1) {
      // Sign up tab
      if (!formData.name) {
        errors.name = 'Name is required';
      }

      if (!formData.confirmPassword) {
        errors.confirmPassword = 'Please confirm your password';
      } else if (formData.password !== formData.confirmPassword) {
        errors.confirmPassword = 'Passwords do not match';
      }
    }

    setFormErrors(errors);
    return Object.keys(errors).length === 0;
  }, [formData, tabValue]);

  const handleEmailAuth = useCallback(async () => {
    if (!validateForm()) return;

    if (tabValue === 0) {
      // Sign in
      await signInWithEmail(formData.email, formData.password);
    } else {
      // Sign up
      await signUp(formData.email, formData.password, formData.name);
    }

    // Only close dialog and call success callback if authentication was successful
    onSuccess?.();
    onClose();
  }, [formData, tabValue, signInWithEmail, signUp, validateForm, onSuccess, onClose]);

  const handleGoogleAuth = useCallback(async () => {
    await signInWithGoogle();
    // Only close dialog and call success callback if authentication was successful
    onSuccess?.();
    onClose();
  }, [signInWithGoogle, onSuccess, onClose]);

  const handleFacebookAuth = useCallback(async () => {
    await signInWithFacebook();
    // Only close dialog and call success callback if authentication was successful
    onSuccess?.();
    onClose();
  }, [signInWithFacebook, onSuccess, onClose]);

  const handleClose = useCallback(() => {
    setFormData({ email: '', password: '', name: '', confirmPassword: '' });
    setFormErrors({});
    setTabValue(0);
    clearError();
    onClose();
  }, [onClose, clearError]);

  const handleKeyPress = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === 'Enter') {
        handleEmailAuth();
      }
    },
    [handleEmailAuth]
  );

  return (
    <Dialog
      open={open}
      onClose={handleClose}
      maxWidth='sm'
      fullWidth
      aria-labelledby='login-dialog-title'
      aria-describedby='login-dialog-description'
      disableEnforceFocus={false}
      disableAutoFocus={false}
      disableRestoreFocus={false}
      PaperProps={{
        sx: { borderRadius: 2 },
      }}
    >
      <DialogTitle id='login-dialog-title' sx={{ pb: 1 }}>
        <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <Typography variant='h5' component='h2'>
            Welcome to EconGraph
          </Typography>
          <IconButton onClick={handleClose} size='small' aria-label='Close dialog'>
            <CloseIcon />
          </IconButton>
        </Box>
        <Typography
          id='login-dialog-description'
          variant='body2'
          color='text.secondary'
          sx={{ mt: 1 }}
        >
          Sign in to access professional chart collaboration features
        </Typography>
      </DialogTitle>

      <DialogContent>
        {error && (
          <Alert severity='error' sx={{ mb: 2 }} onClose={clearError}>
            {error}
          </Alert>
        )}

        {/* OAuth Buttons */}
        <Box sx={{ mb: 3 }}>
          <Button
            variant='outlined'
            fullWidth
            size='large'
            startIcon={<GoogleIcon />}
            onClick={handleGoogleAuth}
            disabled={isLoading}
            sx={{ mb: 1, textTransform: 'none', py: 1.5 }}
          >
            Continue with Google
          </Button>

          <Button
            variant='outlined'
            fullWidth
            size='large'
            startIcon={<FacebookIcon />}
            onClick={handleFacebookAuth}
            disabled={isLoading || !isFacebookAvailable}
            sx={{ textTransform: 'none', py: 1.5 }}
          >
            {isFacebookAvailable ? 'Continue with Facebook' : 'Facebook (Unavailable)'}
          </Button>
        </Box>

        <Divider sx={{ my: 2 }}>
          <Typography variant='body2' color='text.secondary'>
            or
          </Typography>
        </Divider>

        {/* Email/Password Form */}
        <Box>
          <Tabs value={tabValue} onChange={handleTabChange} sx={{ mb: 2 }}>
            <Tab label='Sign In' />
            <Tab label='Sign Up' />
          </Tabs>

          <TabPanel value={tabValue} index={0}>
            {/* Sign In Form */}
            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              <TextField
                label='Email'
                type='email'
                value={formData.email}
                onChange={handleInputChange('email')}
                onKeyPress={handleKeyPress}
                error={!!formErrors.email}
                helperText={formErrors.email}
                fullWidth
                InputProps={{
                  startAdornment: (
                    <InputAdornment position='start'>
                      <EmailIcon color='action' />
                    </InputAdornment>
                  ),
                }}
              />

              <TextField
                label='Password'
                type={showPassword ? 'text' : 'password'}
                value={formData.password}
                onChange={handleInputChange('password')}
                onKeyPress={handleKeyPress}
                error={!!formErrors.password}
                helperText={formErrors.password}
                fullWidth
                InputProps={{
                  startAdornment: (
                    <InputAdornment position='start'>
                      <LockIcon color='action' />
                    </InputAdornment>
                  ),
                  endAdornment: (
                    <InputAdornment position='end'>
                      <IconButton onClick={() => setShowPassword(!showPassword)} edge='end'>
                        {showPassword ? <VisibilityOffIcon /> : <VisibilityIcon />}
                      </IconButton>
                    </InputAdornment>
                  ),
                }}
              />
            </Box>
          </TabPanel>

          <TabPanel value={tabValue} index={1}>
            {/* Sign Up Form */}
            <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              <TextField
                label='Full Name'
                value={formData.name}
                onChange={handleInputChange('name')}
                error={!!formErrors.name}
                helperText={formErrors.name}
                fullWidth
                InputProps={{
                  startAdornment: (
                    <InputAdornment position='start'>
                      <PersonIcon color='action' />
                    </InputAdornment>
                  ),
                }}
              />

              <TextField
                label='Email'
                type='email'
                value={formData.email}
                onChange={handleInputChange('email')}
                error={!!formErrors.email}
                helperText={formErrors.email}
                fullWidth
                InputProps={{
                  startAdornment: (
                    <InputAdornment position='start'>
                      <EmailIcon color='action' />
                    </InputAdornment>
                  ),
                }}
              />

              <TextField
                label='Password'
                type={showPassword ? 'text' : 'password'}
                value={formData.password}
                onChange={handleInputChange('password')}
                error={!!formErrors.password}
                helperText={formErrors.password || 'Minimum 8 characters'}
                fullWidth
                InputProps={{
                  startAdornment: (
                    <InputAdornment position='start'>
                      <LockIcon color='action' />
                    </InputAdornment>
                  ),
                  endAdornment: (
                    <InputAdornment position='end'>
                      <IconButton onClick={() => setShowPassword(!showPassword)} edge='end'>
                        {showPassword ? <VisibilityOffIcon /> : <VisibilityIcon />}
                      </IconButton>
                    </InputAdornment>
                  ),
                }}
              />

              <TextField
                label='Confirm Password'
                type='password'
                value={formData.confirmPassword}
                onChange={handleInputChange('confirmPassword')}
                onKeyPress={handleKeyPress}
                error={!!formErrors.confirmPassword}
                helperText={formErrors.confirmPassword}
                fullWidth
                InputProps={{
                  startAdornment: (
                    <InputAdornment position='start'>
                      <LockIcon color='action' />
                    </InputAdornment>
                  ),
                }}
              />
            </Box>
          </TabPanel>
        </Box>
      </DialogContent>

      <DialogActions sx={{ px: 3, pb: 3 }}>
        <Button
          variant='contained'
          fullWidth
          size='large'
          onClick={handleEmailAuth}
          disabled={isLoading}
          sx={{ py: 1.5 }}
        >
          {isLoading ? (
            <CircularProgress size={24} color='inherit' />
          ) : tabValue === 0 ? (
            'Sign In'
          ) : (
            'Create Account'
          )}
        </Button>
      </DialogActions>

      {tabValue === 1 && (
        <Box sx={{ px: 3, pb: 2 }}>
          <Typography variant='caption' color='text.secondary' align='center' display='block'>
            By creating an account, you agree to our Terms of Service and Privacy Policy. Your data
            is secure and will only be used for chart collaboration features.
          </Typography>
        </Box>
      )}
    </Dialog>
  );
};

export default LoginDialog;
