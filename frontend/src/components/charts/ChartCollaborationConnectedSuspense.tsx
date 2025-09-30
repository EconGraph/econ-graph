/**
 * REQUIREMENT: Enterprise-grade chart collaboration with backend integration
 * PURPOSE: Professional collaboration component connected to GraphQL backend
 * This provides Bloomberg Terminal-level collaboration for institutional users.
 * 
 * SUSPENSE VERSION: Uses React Suspense for better data loading UX
 */

import React, { useState, useCallback, Suspense } from 'react';
import {
  Box,
  Typography,
  TextField,
  Button,
  Avatar,
  List,
  ListItem,
  ListItemAvatar,
  ListItemText,
  ListItemSecondaryAction,
  IconButton,
  Chip,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Drawer,
  Divider,
  Badge,
  Tooltip,
  Alert,
  CircularProgress,
  Snackbar,
} from '@mui/material';
import {
  Add as AddIcon,
  Delete as DeleteIcon,
  Share as ShareIcon,
  Comment as CommentIcon,
  Visibility as VisibilityIcon,
  VisibilityOff as VisibilityOffIcon,
  PushPin as PinIcon,
  Close as CloseIcon,
} from '@mui/icons-material';
import { format } from 'date-fns';
import { useCollaboration } from '../../hooks/useCollaboration';
import { useAuth } from '../../contexts/AuthContext';
import { ChartAnnotationType } from '../../utils/graphql';

// Suspense-compatible data hook that throws promises
const useCollaborationData = (chartId: string) => {
  const collaboration = useCollaboration(chartId);
  
  // If still loading, throw a promise to suspend
  if (collaboration.loading) {
    throw new Promise(resolve => {
      // This will be resolved when the data is ready
      const checkData = () => {
        if (!collaboration.loading) {
          resolve(undefined);
        } else {
          setTimeout(checkData, 100);
        }
      };
      checkData();
    });
  }
  
  // If there's an error, throw it
  if (collaboration.error) {
    throw collaboration.error;
  }
  
  return collaboration;
};

// Main content component that assumes data is available
const ChartCollaborationContent = ({
  chartId,
  isOpen,
  onToggle,
  onAnnotationClick,
  seriesId,
}: {
  chartId: string;
  isOpen: boolean;
  onToggle: () => void;
  onAnnotationClick?: (annotation: ChartAnnotationType) => void;
  seriesId?: string;
}) => {
  const { user: currentUser } = useAuth();
  const collaboration = useCollaborationData(chartId);
  
  const {
    collaborators,
    users,
    annotations,
    comments,
    loadCollaborators,
    loadUsers,
    loadAnnotations,
    loadComments,
    createAnnotation,
    updateAnnotation,
    deleteAnnotation,
    addComment,
    shareChart,
    getCommentsForAnnotation,
    getUserById,
  } = collaboration;

  // State management
  const [filterBy, setFilterBy] = useState<'all' | 'mine' | 'pinned'>('all');
  const [selectedAnnotation, setSelectedAnnotation] = useState<ChartAnnotationType | null>(null);
  const [newAnnotationDialog, setNewAnnotationDialog] = useState(false);
  const [commentsDialog, setCommentsDialog] = useState(false);
  const [shareDialog, setShareDialog] = useState(false);
  const [newComment, setNewComment] = useState('');
  const [snackbar, setSnackbar] = useState<{
    open: boolean;
    message: string;
    severity: 'success' | 'error';
  }>({
    open: false,
    message: '',
    severity: 'success',
  });

  // New annotation form state
  const [annotationForm, setAnnotationForm] = useState({
    annotationDate: new Date().toISOString().split('T')[0],
    annotationValue: '',
    title: '',
    content: '',
    color: '#1976d2',
    annotationType: 'note',
    isPublic: true,
  });

  // Share form state
  const [shareForm, setShareForm] = useState({
    targetUserId: '',
    permissionLevel: 'view',
  });

  const resetAnnotationForm = useCallback(() => {
    setAnnotationForm({
      annotationDate: new Date().toISOString().split('T')[0],
      annotationValue: '',
      title: '',
      content: '',
      color: '#1976d2',
      annotationType: 'note',
      isPublic: true,
    });
  }, []);

  const showSnackbar = useCallback((message: string, severity: 'success' | 'error' = 'success') => {
    setSnackbar({ open: true, message, severity });
  }, []);

  const handleCreateAnnotation = useCallback(async () => {
    if (!annotationForm.title.trim() || !annotationForm.content.trim()) {
      showSnackbar('Title and content are required', 'error');
      return;
    }

    try {
      await createAnnotation({
        chart_id: chartId,
        series_id: seriesId,
        title: annotationForm.title,
        content: annotationForm.content,
        annotation_date: annotationForm.annotationDate,
        annotation_value: annotationForm.annotationValue,
        annotation_type: annotationForm.annotationType,
        color: annotationForm.color,
        is_public: annotationForm.isPublic,
      });

      setNewAnnotationDialog(false);
      resetAnnotationForm();
      showSnackbar('Annotation created successfully');
    } catch (error) {
      showSnackbar(error instanceof Error ? error.message : 'Failed to create annotation', 'error');
    }
  }, [annotationForm, seriesId, createAnnotation, resetAnnotationForm, showSnackbar, chartId]);

  const handleAddComment = useCallback(async () => {
    if (!selectedAnnotation || !newComment.trim()) return;

    try {
      await addComment(selectedAnnotation.id, newComment);
      setNewComment('');
      showSnackbar('Comment added successfully');
    } catch (error) {
      showSnackbar(error instanceof Error ? error.message : 'Failed to add comment', 'error');
    }
  }, [selectedAnnotation, newComment, addComment, showSnackbar]);

  const handleShareChart = useCallback(async () => {
    if (!chartId || !shareForm.targetUserId) {
      showSnackbar('Chart ID and target user are required', 'error');
      return;
    }

    try {
      await shareChart({
        target_user_id: shareForm.targetUserId,
        chart_id: chartId,
        permission_level: shareForm.permissionLevel,
      });

      setShareDialog(false);
      setShareForm({ targetUserId: '', permissionLevel: 'view' });
      showSnackbar('Chart shared successfully');
    } catch (error) {
      showSnackbar(error instanceof Error ? error.message : 'Failed to share chart', 'error');
    }
  }, [chartId, shareForm, shareChart, showSnackbar]);

  const handleDeleteAnnotation = useCallback(
    async (annotationId: string) => {
      if (!window.confirm('Are you sure you want to delete this annotation?')) return;

      try {
        await deleteAnnotation(annotationId);
        showSnackbar('Annotation deleted successfully');
      } catch (error) {
        showSnackbar(
          error instanceof Error ? error.message : 'Failed to delete annotation',
          'error'
        );
      }
    },
    [deleteAnnotation, showSnackbar]
  );

  const handleAnnotationSelect = useCallback(
    async (annotation: ChartAnnotationType) => {
      setSelectedAnnotation(annotation);
      await loadComments(annotation.id);
      if (onAnnotationClick) {
        onAnnotationClick(annotation);
      }
    },
    [loadComments, onAnnotationClick]
  );

  // Filter annotations
  const filteredAnnotations = annotations.filter(annotation => {
    switch (filterBy) {
      case 'mine':
        return annotation.user_id === currentUser?.id;
      case 'pinned':
        return annotation.is_pinned;
      default:
        return annotation.is_visible !== false;
    }
  });

  const totalComments = annotations.reduce((sum, annotation) => {
    const comments = getCommentsForAnnotation(annotation.id);
    return sum + (comments?.length || 0);
  }, 0);

  const activeCollaborators = collaborators.filter(
    c => users[c.user_id] && Date.now() - new Date(c.last_accessed_at || 0).getTime() < 300000 // 5 minutes
  );

  return (
    <>
      {/* Collaboration Panel */}
      <Drawer
        anchor='right'
        open={isOpen}
        onClose={onToggle}
        variant='persistent'
        sx={{
          width: 420,
          flexShrink: 0,
          '& .MuiDrawer-paper': {
            width: 420,
            boxSizing: 'border-box',
            top: 64,
            height: 'calc(100% - 64px)',
            bgcolor: 'background.default',
          },
        }}
      >
        <Box sx={{ p: 2, height: '100%', display: 'flex', flexDirection: 'column' }}>
          {/* Header */}
          <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
            <Typography variant='h6' sx={{ flexGrow: 1 }}>
              Chart Collaboration
            </Typography>
            <Badge badgeContent={totalComments} color='primary'>
              <CommentIcon />
            </Badge>
            <IconButton onClick={onToggle} size='small' sx={{ ml: 1 }}>
              <CloseIcon />
            </IconButton>
          </Box>

          {/* Active Collaborators */}
          {chartId && (
            <Box sx={{ mb: 3 }}>
              <Box
                sx={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  mb: 1,
                }}
              >
                <Typography variant='subtitle2'>
                  Active Collaborators ({activeCollaborators.length})
                </Typography>
                <IconButton
                  size='small'
                  onClick={() => setShareDialog(true)}
                  aria-label='Share chart'
                  data-testid='share-chart-button'
                >
                  <ShareIcon />
                </IconButton>
              </Box>
              <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                {activeCollaborators.slice(0, 8).map(collaborator => {
                  const user = getUserById(collaborator.user_id);
                  return (
                    <Tooltip
                      key={collaborator.id}
                      title={user ? `${user.name} (${collaborator.role})` : 'Loading...'}
                    >
                      <Badge
                        color='success'
                        variant='dot'
                        anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
                      >
                        <Avatar src={user?.avatarUrl} sx={{ width: 32, height: 32 }}>
                          {user?.name?.[0] || '?'}
                        </Avatar>
                      </Badge>
                    </Tooltip>
                  );
                })}
                {activeCollaborators.length > 8 && (
                  <Avatar sx={{ width: 32, height: 32 }}>+{activeCollaborators.length - 8}</Avatar>
                )}
              </Box>
            </Box>
          )}

          <Divider sx={{ mb: 2 }} />

          {/* Controls */}
          <Box sx={{ display: 'flex', gap: 1, mb: 2 }}>
            <Button
              variant='contained'
              startIcon={<AddIcon />}
              onClick={() => setNewAnnotationDialog(true)}
              size='small'
              fullWidth
            >
              Add Annotation
            </Button>
          </Box>

          {/* Filter */}
          <FormControl size='small' fullWidth sx={{ mb: 2 }}>
            <InputLabel id='filter-annotations-label'>Filter Annotations</InputLabel>
            <Select
              labelId='filter-annotations-label'
              value={filterBy}
              onChange={e => setFilterBy(e.target.value as any)}
              label='Filter Annotations'
              data-testid='filter-annotations-select'
              inputProps={{
                'aria-label': 'Filter annotations by type',
              }}
              MenuProps={{
                disablePortal: true,
                container: document.body,
              }}
            >
              <MenuItem value='all'>All Annotations ({annotations.length})</MenuItem>
              <MenuItem value='mine'>
                My Annotations ({annotations.filter(a => a.user_id === currentUser?.id).length})
              </MenuItem>
              <MenuItem value='pinned'>
                Pinned ({annotations.filter(a => a.is_pinned).length})
              </MenuItem>
            </Select>
          </FormControl>

          {/* Annotations List */}
          <Box sx={{ flexGrow: 1, overflow: 'auto' }}>
            <Typography variant='subtitle2' sx={{ mb: 1 }}>
              Annotations ({filteredAnnotations.length})
            </Typography>
            {filteredAnnotations.length === 0 ? (
              <Typography variant='body2' color='text.secondary'>
                No annotations found. Click "Add Annotation" to create one.
              </Typography>
            ) : (
              <List dense>
                {filteredAnnotations.map(annotation => {
                  const annotationComments = getCommentsForAnnotation(annotation.id);
                  return (
                    <ListItem
                      key={annotation.id}
                      sx={{
                        border: '1px solid',
                        borderColor: 'divider',
                        borderRadius: 1,
                        mb: 1,
                        bgcolor: 'background.paper',
                      }}
                    >
                      <ListItemAvatar>
                        <Avatar
                          sx={{
                            bgcolor: annotation.color || 'primary.main',
                            width: 32,
                            height: 32,
                          }}
                        >
                          {annotation.annotation_type?.[0]?.toUpperCase() || 'N'}
                        </Avatar>
                      </ListItemAvatar>
                      <ListItemText
                        primary={
                          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                            <Typography variant='body2' fontWeight='medium'>
                              {annotation.title}
                            </Typography>
                            {annotation.is_pinned && (
                              <Chip
                                icon={<PinIcon />}
                                label='Pinned'
                                size='small'
                                color='primary'
                                variant='outlined'
                              />
                            )}
                            {!annotation.is_public && (
                              <Chip
                                icon={<VisibilityOffIcon />}
                                label='Private'
                                size='small'
                                color='secondary'
                                variant='outlined'
                              />
                            )}
                          </Box>
                        }
                        secondary={
                          <Box>
                            <Typography variant='caption' color='text.secondary'>
                              {annotation.content}
                            </Typography>
                            <Typography variant='caption' display='block' color='text.secondary'>
                              {format(new Date(annotation.created_at), 'MMM dd, yyyy HH:mm')}
                            </Typography>
                          </Box>
                        }
                      />
                      <ListItemSecondaryAction>
                        <Box sx={{ display: 'flex', gap: 0.5 }}>
                          <IconButton
                            size='small'
                            onClick={() => {
                              handleAnnotationSelect(annotation);
                              setCommentsDialog(true);
                            }}
                            data-testid='comment-button'
                          >
                            <Badge badgeContent={annotationComments?.length || 0} color='primary'>
                              <CommentIcon />
                            </Badge>
                          </IconButton>
                          <IconButton
                            size='small'
                            onClick={() => handleDeleteAnnotation(annotation.id)}
                            data-testid={`delete-annotation-${annotation.id}`}
                          >
                            <DeleteIcon />
                          </IconButton>
                        </Box>
                      </ListItemSecondaryAction>
                    </ListItem>
                  );
                })}
              </List>
            )}
          </Box>
        </Box>
      </Drawer>

      {/* New Annotation Dialog */}
      <Dialog open={newAnnotationDialog} onClose={() => setNewAnnotationDialog(false)} maxWidth='sm' fullWidth>
        <DialogTitle>Add New Annotation</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, mt: 1 }}>
            <TextField
              label='Title'
              value={annotationForm.title}
              onChange={e => setAnnotationForm(prev => ({ ...prev, title: e.target.value }))}
              fullWidth
              required
            />
            <TextField
              label='Content'
              value={annotationForm.content}
              onChange={e => setAnnotationForm(prev => ({ ...prev, content: e.target.value }))}
              multiline
              rows={3}
              fullWidth
              required
            />
            <Box sx={{ display: 'flex', gap: 2 }}>
              <TextField
                label='Date'
                type='date'
                value={annotationForm.annotationDate}
                onChange={e => setAnnotationForm(prev => ({ ...prev, annotationDate: e.target.value }))}
                InputLabelProps={{ shrink: true }}
                sx={{ flex: 1 }}
              />
              <TextField
                label='Value'
                value={annotationForm.annotationValue}
                onChange={e => setAnnotationForm(prev => ({ ...prev, annotationValue: e.target.value }))}
                sx={{ flex: 1 }}
              />
            </Box>
            <Box sx={{ display: 'flex', gap: 2 }}>
              <FormControl sx={{ flex: 1 }}>
                <InputLabel>Type</InputLabel>
                <Select
                  value={annotationForm.annotationType}
                  onChange={e => setAnnotationForm(prev => ({ ...prev, annotationType: e.target.value }))}
                  label='Type'
                >
                  <MenuItem value='note'>Note</MenuItem>
                  <MenuItem value='warning'>Warning</MenuItem>
                  <MenuItem value='info'>Info</MenuItem>
                </Select>
              </FormControl>
              <TextField
                label='Color'
                type='color'
                value={annotationForm.color}
                onChange={e => setAnnotationForm(prev => ({ ...prev, color: e.target.value }))}
                sx={{ width: 100 }}
              />
            </Box>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setNewAnnotationDialog(false)}>Cancel</Button>
          <Button onClick={handleCreateAnnotation} variant='contained'>
            Create Annotation
          </Button>
        </DialogActions>
      </Dialog>

      {/* Comments Dialog */}
      <Dialog open={commentsDialog} onClose={() => setCommentsDialog(false)} maxWidth='sm' fullWidth>
        <DialogTitle>
          Comments for "{selectedAnnotation?.title}"
          <IconButton
            onClick={() => setCommentsDialog(false)}
            sx={{ position: 'absolute', right: 8, top: 8 }}
          >
            <CloseIcon />
          </IconButton>
        </DialogTitle>
        <DialogContent>
          <Box sx={{ mt: 2 }}>
            {selectedAnnotation && getCommentsForAnnotation(selectedAnnotation.id)?.map(comment => (
              <Box key={comment.id} sx={{ mb: 2, p: 2, bgcolor: 'grey.50', borderRadius: 1 }}>
                <Typography variant='body2'>{comment.content}</Typography>
                <Typography variant='caption' color='text.secondary'>
                  {comment.user_name} • {format(new Date(comment.created_at), 'MMM dd, yyyy HH:mm')}
                </Typography>
              </Box>
            ))}
            <TextField
              label='Add a comment'
              value={newComment}
              onChange={e => setNewComment(e.target.value)}
              multiline
              rows={2}
              fullWidth
              sx={{ mt: 2 }}
            />
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setCommentsDialog(false)}>Close</Button>
          <Button onClick={handleAddComment} variant='contained' disabled={!newComment.trim()}>
            Add Comment
          </Button>
        </DialogActions>
      </Dialog>

      {/* Share Dialog */}
      <Dialog open={shareDialog} onClose={() => setShareDialog(false)} maxWidth='sm' fullWidth>
        <DialogTitle>Share Chart</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, mt: 1 }}>
            <FormControl fullWidth>
              <InputLabel>Target User</InputLabel>
              <Select
                value={shareForm.targetUserId}
                onChange={e => setShareForm(prev => ({ ...prev, targetUserId: e.target.value }))}
                label='Target User'
              >
                {Object.values(users).map(user => (
                  <MenuItem key={user.id} value={user.id}>
                    {user.name} ({user.email})
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
            <FormControl fullWidth>
              <InputLabel>Permission Level</InputLabel>
              <Select
                value={shareForm.permissionLevel}
                onChange={e => setShareForm(prev => ({ ...prev, permissionLevel: e.target.value }))}
                label='Permission Level'
              >
                <MenuItem value='view'>View Only</MenuItem>
                <MenuItem value='comment'>Comment</MenuItem>
                <MenuItem value='edit'>Edit</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setShareDialog(false)}>Cancel</Button>
          <Button onClick={handleShareChart} variant='contained'>
            Share Chart
          </Button>
        </DialogActions>
      </Dialog>

      {/* Snackbar */}
      <Snackbar
        open={snackbar.open}
        autoHideDuration={6000}
        onClose={() => setSnackbar(prev => ({ ...prev, open: false }))}
        message={snackbar.message}
      />
    </>
  );
};

// Main component with Suspense boundary
export const ChartCollaborationConnectedSuspense = ({
  chartId,
  isOpen,
  onToggle,
  onAnnotationClick,
  seriesId,
}: {
  chartId: string;
  isOpen: boolean;
  onToggle: () => void;
  onAnnotationClick?: (annotation: ChartAnnotationType) => void;
  seriesId?: string;
}) => {
  const { user: currentUser } = useAuth();

  if (!currentUser) {
    return (
      <Alert severity='warning' sx={{ m: 2 }}>
        Please log in to access collaboration features.
      </Alert>
    );
  }

  return (
    <Suspense 
      fallback={
        <Box sx={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: 200 }}>
          <CircularProgress />
        </Box>
      }
    >
      <ChartCollaborationContent 
        chartId={chartId} 
        isOpen={isOpen} 
        onToggle={onToggle}
        onAnnotationClick={onAnnotationClick}
        seriesId={seriesId}
      />
    </Suspense>
  );
};