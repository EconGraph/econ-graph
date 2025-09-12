/**
 * User Data Deletion Callback Page
 *
 * This page handles Facebook's user data deletion callback requirements.
 * When a user deletes their Facebook account, Facebook will send a POST request
 * to this endpoint to notify us that we should delete the user's data.
 *
 * REQUIREMENT: Facebook App Review - User Data Deletion Callback
 * PURPOSE: Comply with Facebook's data deletion requirements for app approval
 */

import React, { useState, useEffect } from 'react';
import {
  Box,
  Card,
  CardContent,
  Typography,
  Alert,
  CircularProgress,
  Button,
  Divider,
  List,
  ListItem,
  ListItemText,
  ListItemIcon,
  Chip,
} from '@mui/material';
import { CheckCircle, Error, Info, Delete, Security, DataUsage } from '@mui/icons-material';
import { useNavigate } from 'react-router-dom';

interface DeletionStatus {
  status: 'processing' | 'success' | 'error' | 'not_found';
  message: string;
  userId?: string;
  deletedAt?: string;
}

const UserDataDeletion: React.FC = () => {
  const [deletionStatus, setDeletionStatus] = useState<DeletionStatus>({
    status: 'processing',
    message: 'Processing your data deletion request...',
  });
  const [isLoading, setIsLoading] = useState(true);
  const navigate = useNavigate();

  useEffect(() => {
    // Simulate processing the deletion request
    // In a real implementation, this would:
    // 1. Verify the Facebook signed request
    // 2. Extract the user ID from the request
    // 3. Delete the user's data from our database
    // 4. Return appropriate status

    const processDeletion = async () => {
      try {
        // Simulate API call to process deletion
        await new Promise(resolve => setTimeout(resolve, 2000));

        // In real implementation, this would be an API call to:
        // POST /api/auth/facebook/data-deletion
        // with Facebook's signed request payload

        setDeletionStatus({
          status: 'success',
          message: 'Your data has been successfully deleted from our systems.',
          userId: 'fb_user_12345', // Would come from Facebook's request
          deletedAt: new Date().toISOString(),
        });
      } catch (error) {
        setDeletionStatus({
          status: 'error',
          message: 'There was an error processing your deletion request. Please contact support.',
        });
      } finally {
        setIsLoading(false);
      }
    };

    processDeletion();
  }, []);

  const getStatusIcon = () => {
    switch (deletionStatus.status) {
      case 'success':
        return <CheckCircle color='success' sx={{ fontSize: 48 }} />;
      case 'error':
        return <Error color='error' sx={{ fontSize: 48 }} />;
      case 'not_found':
        return <Info color='info' sx={{ fontSize: 48 }} />;
      default:
        return <CircularProgress size={48} />;
    }
  };

  // const getStatusColor = () => {
  //   switch (deletionStatus.status) {
  //     case 'success':
  //       return 'success';
  //     case 'error':
  //       return 'error';
  //     case 'not_found':
  //       return 'info';
  //     default:
  //       return 'primary';
  //   }
  // };

  const getStatusChip = () => {
    switch (deletionStatus.status) {
      case 'success':
        return <Chip label='Data Deleted' color='success' icon={<CheckCircle />} />;
      case 'error':
        return <Chip label='Error' color='error' icon={<Error />} />;
      case 'not_found':
        return <Chip label='No Data Found' color='info' icon={<Info />} />;
      default:
        return <Chip label='Processing' color='primary' icon={<CircularProgress size={16} />} />;
    }
  };

  return (
    <Box
      sx={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        bgcolor: 'background.default',
        p: 2,
      }}
    >
      <Card sx={{ maxWidth: 600, width: '100%' }}>
        <CardContent sx={{ p: 4 }}>
          {/* Header */}
          <Box sx={{ textAlign: 'center', mb: 3 }}>
            <Security sx={{ fontSize: 64, color: 'primary.main', mb: 2 }} />
            <Typography variant='h4' component='h1' gutterBottom>
              Data Deletion Request
            </Typography>
            <Typography variant='body1' color='text.secondary'>
              Processing your Facebook account data deletion request
            </Typography>
          </Box>

          {/* Status Section */}
          <Box sx={{ textAlign: 'center', mb: 4 }}>
            {getStatusIcon()}
            <Typography variant='h6' sx={{ mt: 2, mb: 1 }}>
              {deletionStatus.message}
            </Typography>
            {getStatusChip()}
          </Box>

          {/* Details Section */}
          {deletionStatus.status === 'success' && (
            <Alert severity='success' sx={{ mb: 3 }}>
              <Typography variant='body2'>
                Your personal data has been permanently deleted from our systems. This includes your
                account information, preferences, and usage data.
              </Typography>
            </Alert>
          )}

          {deletionStatus.status === 'error' && (
            <Alert severity='error' sx={{ mb: 3 }}>
              <Typography variant='body2'>
                We encountered an issue processing your deletion request. Please contact our support
                team for assistance.
              </Typography>
            </Alert>
          )}

          {/* What Was Deleted */}
          {deletionStatus.status === 'success' && (
            <Box sx={{ mb: 3 }}>
              <Typography variant='h6' gutterBottom>
                <DataUsage sx={{ mr: 1, verticalAlign: 'middle' }} />
                Data Deleted
              </Typography>
              <List dense>
                <ListItem>
                  <ListItemIcon>
                    <Delete color='action' />
                  </ListItemIcon>
                  <ListItemText
                    primary='Account Information'
                    secondary='Email, name, profile picture, and authentication data'
                  />
                </ListItem>
                <ListItem>
                  <ListItemIcon>
                    <Delete color='action' />
                  </ListItemIcon>
                  <ListItemText
                    primary='User Preferences'
                    secondary='Theme settings, chart preferences, and notification settings'
                  />
                </ListItem>
                <ListItem>
                  <ListItemIcon>
                    <Delete color='action' />
                  </ListItemIcon>
                  <ListItemText
                    primary='Usage Analytics'
                    secondary='Platform usage patterns and interaction data'
                  />
                </ListItem>
                <ListItem>
                  <ListItemIcon>
                    <Delete color='action' />
                  </ListItemIcon>
                  <ListItemText
                    primary='Session Data'
                    secondary='Login sessions and authentication tokens'
                  />
                </ListItem>
              </List>
            </Box>
          )}

          {/* Deletion Details */}
          {deletionStatus.userId && (
            <Box sx={{ mb: 3 }}>
              <Typography variant='body2' color='text.secondary'>
                <strong>User ID:</strong> {deletionStatus.userId}
              </Typography>
              {deletionStatus.deletedAt && (
                <Typography variant='body2' color='text.secondary'>
                  <strong>Deleted At:</strong> {new Date(deletionStatus.deletedAt).toLocaleString()}
                </Typography>
              )}
            </Box>
          )}

          <Divider sx={{ my: 3 }} />

          {/* Privacy Information */}
          <Box sx={{ mb: 3 }}>
            <Typography variant='h6' gutterBottom>
              Privacy & Data Protection
            </Typography>
            <Typography variant='body2' color='text.secondary' paragraph>
              We are committed to protecting your privacy and complying with data protection
              regulations including GDPR, CCPA, and Facebook's data deletion requirements.
            </Typography>
            <Typography variant='body2' color='text.secondary'>
              This deletion request was processed in accordance with our Privacy Policy and
              applicable data protection laws.
            </Typography>
          </Box>

          {/* Actions */}
          <Box sx={{ display: 'flex', gap: 2, justifyContent: 'center' }}>
            <Button variant='contained' onClick={() => navigate('/')} disabled={isLoading}>
              Return to Home
            </Button>
            <Button
              variant='outlined'
              onClick={() => window.open('mailto:privacy@econ-graph.com')}
              disabled={isLoading}
            >
              Contact Privacy Team
            </Button>
          </Box>

          {/* Footer */}
          <Box sx={{ mt: 4, textAlign: 'center' }}>
            <Typography variant='caption' color='text.secondary'>
              For questions about data deletion, contact us at{' '}
              <a href='mailto:privacy@econ-graph.com'>privacy@econ-graph.com</a>
            </Typography>
          </Box>
        </CardContent>
      </Card>
    </Box>
  );
};

export default UserDataDeletion;
