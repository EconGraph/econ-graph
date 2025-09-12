/**
 * REQUIREMENT: User profile management for authenticated collaboration
 * PURPOSE: Provide user account management and preferences for chart collaboration
 * This enables personalized professional economic analysis experience
 */

import React, { useState, useCallback } from 'react';
import {
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  Button,
  TextField,
  Box,
  Typography,
  Avatar,
  Switch,
  FormControlLabel,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Alert,
  Card,
  CardContent,
  Chip,
  IconButton,
  List,
  ListItem,
  ListItemText,
} from '@mui/material';
import {
  Edit as EditIcon,
  Save as SaveIcon,
  Cancel as CancelIcon,
  Person as PersonIcon,
  Email as EmailIcon,
  Business as BusinessIcon,
  Palette as PaletteIcon,
  Security as SecurityIcon,
  ExitToApp as SignOutIcon,
  Delete as DeleteIcon,
} from '@mui/icons-material';
import { useAuth, User } from '../../contexts/AuthContext';
import { useTheme } from '../../contexts/ThemeContext';

interface UserProfileProps {
  open: boolean;
  onClose: () => void;
}

const UserProfile: React.FC<UserProfileProps> = ({ open, onClose }) => {
  const { user, updateProfile, signOut, error, clearError } = useAuth();
  const { setTheme, currentTheme } = useTheme();
  const [isEditing, setIsEditing] = useState(false);
  const [formData, setFormData] = useState<Partial<User>>({});
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  React.useEffect(() => {
    if (user) {
      setFormData({
        name: user.name,
        email: user.email,
        organization: user.organization,
        preferences: { ...user.preferences },
      });
    }
  }, [user]);

  const handleInputChange = useCallback(
    (field: string) => (event: React.ChangeEvent<HTMLInputElement>) => {
      setFormData(prev => ({
        ...prev,
        [field]: event.target.value,
      }));
    },
    []
  );

  const handlePreferenceChange = useCallback(
    (preference: string) => (event: React.ChangeEvent<HTMLInputElement>) => {
      setFormData(prev => ({
        ...prev,
        preferences: {
          ...prev.preferences,
          [preference]: event.target.checked,
        } as User['preferences'],
      }));
    },
    []
  );

  const handleSelectChange = useCallback(
    (field: string) => (event: any) => {
      if (field.startsWith('preferences.')) {
        const prefField = field.replace('preferences.', '');
        const newValue = event.target.value;

        setFormData(prev => ({
          ...prev,
          preferences: {
            ...prev.preferences,
            [prefField]: newValue,
          } as User['preferences'],
        }));

        // If theme is being changed, update it immediately
        if (prefField === 'theme') {
          setTheme(newValue as 'light' | 'dark');
        }
      } else {
        setFormData(prev => ({
          ...prev,
          [field]: event.target.value,
        }));
      }
    },
    [setTheme]
  );

  const handleSave = useCallback(async () => {
    try {
      await updateProfile(formData);
      setIsEditing(false);
    } catch (error) {
      // Error is handled by the auth context
    }
  }, [formData, updateProfile]);

  const handleCancel = useCallback(() => {
    if (user) {
      setFormData({
        name: user.name,
        email: user.email,
        organization: user.organization,
        preferences: { ...user.preferences },
      });
    }
    setIsEditing(false);
    clearError();
  }, [user, clearError]);

  const handleSignOut = useCallback(async () => {
    try {
      await signOut();
      onClose();
    } catch (error) {
      // Error is handled by the auth context
    }
  }, [signOut, onClose]);

  const handleDeleteAccount = useCallback(() => {
    // In a real app, this would call a delete account API
    console.log('Delete account requested');
    setShowDeleteConfirm(false);
  }, []);

  if (!user) {
    return null;
  }

  const getRoleBadgeColor = (role: string) => {
    switch (role) {
      case 'admin':
        return 'error';
      case 'analyst':
        return 'primary';
      case 'viewer':
        return 'default';
      default:
        return 'default';
    }
  };

  return (
    <>
      <Dialog
        open={open}
        onClose={onClose}
        maxWidth='md'
        fullWidth
        aria-labelledby='user-profile-title'
        aria-describedby='user-profile-description'
        disableEnforceFocus={false}
        disableAutoFocus={false}
        disableRestoreFocus={false}
      >
        <DialogTitle id='user-profile-title'>
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
            <Avatar src={user.avatar} sx={{ width: 56, height: 56 }}>
              {user.name[0]}
            </Avatar>
            <Box>
              <Typography variant='h5'>User Profile</Typography>
              <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mt: 0.5 }}>
                <Chip label={user.role} color={getRoleBadgeColor(user.role)} size='small' />
                <Chip label={user.provider} variant='outlined' size='small' />
              </Box>
            </Box>
          </Box>
        </DialogTitle>

        <DialogContent>
          {error && (
            <Alert severity='error' sx={{ mb: 2 }} onClose={clearError}>
              {error}
            </Alert>
          )}

          {/* Basic Information */}
          <Card sx={{ mb: 2 }}>
            <CardContent>
              <Box
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  mb: 2,
                }}
              >
                <Typography variant='h6'>Basic Information</Typography>
                {!isEditing && (
                  <IconButton onClick={() => setIsEditing(true)}>
                    <EditIcon />
                  </IconButton>
                )}
              </Box>

              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <TextField
                  label='Full Name'
                  value={formData.name || ''}
                  onChange={handleInputChange('name')}
                  disabled={!isEditing}
                  fullWidth
                  InputProps={{
                    startAdornment: <PersonIcon color='action' sx={{ mr: 1 }} />,
                  }}
                />

                <TextField
                  label='Email'
                  value={formData.email || ''}
                  onChange={handleInputChange('email')}
                  disabled={true} // Email usually can't be changed
                  fullWidth
                  InputProps={{
                    startAdornment: <EmailIcon color='action' sx={{ mr: 1 }} />,
                  }}
                />

                <TextField
                  label='Organization (Optional)'
                  value={formData.organization || ''}
                  onChange={handleInputChange('organization')}
                  disabled={!isEditing}
                  fullWidth
                  InputProps={{
                    startAdornment: <BusinessIcon color='action' sx={{ mr: 1 }} />,
                  }}
                />
              </Box>
            </CardContent>
          </Card>

          {/* Preferences */}
          <Card sx={{ mb: 2 }}>
            <CardContent>
              <Typography variant='h6' sx={{ mb: 2, display: 'flex', alignItems: 'center' }}>
                <PaletteIcon sx={{ mr: 1 }} />
                Preferences
              </Typography>

              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <FormControl fullWidth>
                  <InputLabel id='theme-select-label'>Theme</InputLabel>
                  <Select
                    labelId='theme-select-label'
                    id='theme-select'
                    value={formData.preferences?.theme || currentTheme}
                    onChange={handleSelectChange('preferences.theme')}
                    label='Theme'
                  >
                    <MenuItem value='light'>Light</MenuItem>
                    <MenuItem value='dark'>Dark</MenuItem>
                  </Select>
                </FormControl>

                <FormControl fullWidth>
                  <InputLabel id='chart-type-select-label'>Default Chart Type</InputLabel>
                  <Select
                    labelId='chart-type-select-label'
                    id='chart-type-select'
                    value={formData.preferences?.defaultChartType || 'line'}
                    onChange={handleSelectChange('preferences.defaultChartType')}
                    label='Default Chart Type'
                  >
                    <MenuItem value='line'>Line Chart</MenuItem>
                    <MenuItem value='area'>Area Chart</MenuItem>
                    <MenuItem value='bar'>Bar Chart</MenuItem>
                    <MenuItem value='candlestick'>Candlestick</MenuItem>
                  </Select>
                </FormControl>

                <FormControlLabel
                  control={
                    <Switch
                      checked={formData.preferences?.notifications ?? true}
                      onChange={handlePreferenceChange('notifications')}
                    />
                  }
                  label='Email Notifications'
                />

                <FormControlLabel
                  control={
                    <Switch
                      checked={formData.preferences?.collaborationEnabled ?? true}
                      onChange={handlePreferenceChange('collaborationEnabled')}
                    />
                  }
                  label='Enable Chart Collaboration'
                />
              </Box>

              {/* Save Preferences Button */}
              <Box sx={{ mt: 2, display: 'flex', justifyContent: 'flex-end' }}>
                <Button
                  variant='contained'
                  startIcon={<SaveIcon />}
                  onClick={handleSave}
                  size='small'
                >
                  Save Preferences
                </Button>
              </Box>
            </CardContent>
          </Card>

          {/* Account Information */}
          <Card sx={{ mb: 2 }}>
            <CardContent>
              <Typography variant='h6' sx={{ mb: 2, display: 'flex', alignItems: 'center' }}>
                <SecurityIcon sx={{ mr: 1 }} />
                Account Information
              </Typography>

              <List dense>
                <ListItem>
                  <ListItemText
                    primary='Account Created'
                    secondary={new Date(user.createdAt).toLocaleDateString()}
                  />
                </ListItem>
                <ListItem>
                  <ListItemText
                    primary='Last Login'
                    secondary={new Date(user.lastLoginAt).toLocaleDateString()}
                  />
                </ListItem>
                <ListItem>
                  <ListItemText
                    primary='Authentication Provider'
                    secondary={user.provider.charAt(0).toUpperCase() + user.provider.slice(1)}
                  />
                </ListItem>
                <ListItem>
                  <ListItemText
                    primary='Account Role'
                    secondary={user.role.charAt(0).toUpperCase() + user.role.slice(1)}
                  />
                </ListItem>
              </List>
            </CardContent>
          </Card>

          {/* Danger Zone */}
          <Card sx={{ border: 1, borderColor: 'error.main' }}>
            <CardContent>
              <Typography variant='h6' color='error' sx={{ mb: 2 }}>
                Danger Zone
              </Typography>

              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
                <Button
                  variant='outlined'
                  color='warning'
                  startIcon={<SignOutIcon />}
                  onClick={handleSignOut}
                  fullWidth
                >
                  Sign Out
                </Button>

                <Button
                  variant='outlined'
                  color='error'
                  startIcon={<DeleteIcon />}
                  onClick={() => setShowDeleteConfirm(true)}
                  fullWidth
                >
                  Delete Account
                </Button>
              </Box>
            </CardContent>
          </Card>
        </DialogContent>

        <DialogActions>
          {isEditing ? (
            <>
              <Button onClick={handleCancel} startIcon={<CancelIcon />}>
                Cancel
              </Button>
              <Button onClick={handleSave} variant='contained' startIcon={<SaveIcon />}>
                Save Changes
              </Button>
            </>
          ) : (
            <Button onClick={onClose}>Close</Button>
          )}
        </DialogActions>
      </Dialog>

      {/* Delete Account Confirmation */}
      <Dialog
        open={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        aria-labelledby='delete-dialog-title'
        aria-describedby='delete-dialog-description'
        disableEnforceFocus={false}
        disableAutoFocus={false}
        disableRestoreFocus={false}
      >
        <DialogTitle id='delete-dialog-title' color='error'>
          Delete Account
        </DialogTitle>
        <DialogContent>
          <Alert severity='error' sx={{ mb: 2 }}>
            This action cannot be undone!
          </Alert>
          <Typography id='delete-dialog-description'>
            Are you sure you want to delete your account? This will permanently remove:
          </Typography>
          <Box component='ul' sx={{ mt: 1 }}>
            <li>Your profile and preferences</li>
            <li>All chart annotations and comments</li>
            <li>Collaboration history</li>
            <li>Saved analyses and dashboards</li>
          </Box>
          <Typography sx={{ mt: 2 }}>
            Type <strong>DELETE</strong> to confirm:
          </Typography>
          <TextField fullWidth sx={{ mt: 1 }} placeholder='Type DELETE to confirm' />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setShowDeleteConfirm(false)}>Cancel</Button>
          <Button onClick={handleDeleteAccount} color='error' variant='contained'>
            Delete Account
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

export default UserProfile;
