/**
 * REQUIREMENT: Enterprise-grade chart collaboration with backend integration
 * PURPOSE: Professional collaboration component connected to GraphQL backend
 * This provides Bloomberg Terminal-level collaboration for institutional users.
 */

import React, { useState, useCallback, useEffect, Suspense } from 'react';
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

interface ChartCollaborationConnectedProps {
  seriesId: string;
  chartId?: string;
  isOpen: boolean;
  onToggle: () => void;
  onAnnotationClick?: (annotation: ChartAnnotationType) => void;
}

const ANNOTATION_COLORS = [
  '#f44336',
  '#e91e63',
  '#9c27b0',
  '#673ab7',
  '#3f51b5',
  '#2196f3',
  '#03a9f4',
  '#00bcd4',
  '#009688',
  '#4caf50',
  '#8bc34a',
  '#cddc39',
  '#ffeb3b',
  '#ffc107',
  '#ff9800',
  '#ff5722',
];

const ANNOTATION_TYPES = [
  { value: 'note', label: 'Note', icon: '📝' },
  { value: 'highlight', label: 'Highlight', icon: '✨' },
  { value: 'warning', label: 'Warning', icon: '⚠️' },
  { value: 'analysis', label: 'Analysis', icon: '📊' },
  { value: 'trend', label: 'Trend', icon: '📈' },
  { value: 'support', label: 'Support Level', icon: '🔹' },
  { value: 'resistance', label: 'Resistance Level', icon: '🔸' },
];

const ChartCollaborationConnected: React.FC<ChartCollaborationConnectedProps> = ({
  seriesId,
  chartId,
  isOpen,
  onToggle,
  onAnnotationClick,
}) => {
  const { user: currentUser } = useAuth();
  const {
    annotations,
    collaborators,
    users,
    loading,
    error,
    createAnnotation,
    addComment,
    shareChart,
    deleteAnnotation,
    toggleAnnotationVisibility,
    toggleAnnotationPin,
    loadComments,
    loadCollaborators,
    loadUsers,
    getUserById,
    getCommentsForAnnotation,
  } = useCollaboration({
    seriesId,
    chartId,
    autoRefresh: true,
    refreshInterval: 30000,
  });

  // Local state
  const [newAnnotationDialog, setNewAnnotationDialog] = useState(false);
  const [selectedAnnotation, setSelectedAnnotation] = useState<ChartAnnotationType | null>(null);
  const [shareDialog, setShareDialog] = useState(false);
  const [newComment, setNewComment] = useState('');
  const [filterBy, setFilterBy] = useState<'all' | 'mine' | 'pinned'>('all');
  const [snackbar, setSnackbar] = useState<{
    open: boolean;
    message: string;
    severity: 'success' | 'error';
  }>({
    open: false,
    message: '',
    severity: 'success',
  });

  // Load collaborators when component mounts
  useEffect(() => {
    if (chartId) {
      loadCollaborators();
    }
  }, [chartId]); // Remove loadCollaborators from dependencies to prevent infinite loop

  // Load users when collaborators change - but only if we have collaborators and haven't loaded users yet
  useEffect(() => {
    if (collaborators.length > 0) {
      const userIds = collaborators.map(c => c.user_id);
      const missingUsers = userIds.filter(id => !users[id]);
      if (missingUsers.length > 0) {
        loadUsers(missingUsers);
      }
    }
  }, [collaborators, loadUsers, users]);

  // New annotation form state
  const [annotationForm, setAnnotationForm] = useState({
    annotationDate: new Date().toISOString().split('T')[0],
    annotationValue: '',
    title: '',
    content: '',
    color: ANNOTATION_COLORS[0],
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
      color: ANNOTATION_COLORS[0],
      annotationType: 'note',
      isPublic: true,
    });
  }, []);

  const showSnackbar = useCallback((message: string, severity: 'success' | 'error' = 'success') => {
    setSnackbar({ open: true, message, severity });
  }, []);

  const handleCreateAnnotation = useCallback(async () => {
    if (!annotationForm.title || !annotationForm.content) {
      showSnackbar('Title and content are required', 'error');
      return;
    }

    try {
      await createAnnotation({
        series_id: seriesId,
        annotation_date: annotationForm.annotationDate,
        annotation_value: annotationForm.annotationValue
          ? parseFloat(annotationForm.annotationValue)
          : undefined,
        title: annotationForm.title,
        content: annotationForm.content,
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
  }, [annotationForm, seriesId, createAnnotation, resetAnnotationForm, showSnackbar]);

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

  if (!currentUser) {
    return (
      <Alert severity='warning' sx={{ m: 2 }}>
        Please log in to access collaboration features.
      </Alert>
    );
  }

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

          {error && (
            <Alert
              severity='error'
              sx={{ mb: 2 }}
              onClose={() => {
                // Mock close handler
              }}
            >
              {error}
            </Alert>
          )}

          {/* Collaborators */}
          {chartId && !loading && (
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
              disabled={loading}
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
                My Annotations ({annotations.filter(a => a.user_id === currentUser.id).length})
              </MenuItem>
              <MenuItem value='pinned'>
                Pinned ({annotations.filter(a => a.is_pinned).length})
              </MenuItem>
            </Select>
          </FormControl>

          {/* Loading */}
          {loading && (
            <Box sx={{ display: 'flex', justifyContent: 'center', my: 2 }}>
              <CircularProgress size={24} />
            </Box>
          )}

          {/* Annotations List */}
          <Typography variant='subtitle2' sx={{ mb: 1 }}>
            Annotations ({filteredAnnotations.length})
          </Typography>

          <Box sx={{ flexGrow: 1, overflow: 'auto' }}>
            <List>
              {filteredAnnotations.map(annotation => {
                const author = getUserById(annotation.user_id);
                const annotationType = ANNOTATION_TYPES.find(
                  t => t.value === annotation.annotation_type
                );
                const commentCount = getCommentsForAnnotation(annotation.id)?.length || 0;

                return (
                  <ListItem
                    key={annotation.id}
                    sx={{
                      border: 1,
                      borderColor: 'divider',
                      borderRadius: 1,
                      mb: 1,
                      bgcolor: annotation.is_pinned ? 'action.hover' : 'background.paper',
                      cursor: 'pointer',
                    }}
                    onClick={() => handleAnnotationSelect(annotation)}
                  >
                    <ListItemAvatar>
                      <Avatar
                        src={author?.avatarUrl}
                        sx={{
                          bgcolor: annotation.color || '#1976d2',
                          width: 32,
                          height: 32,
                        }}
                      >
                        {annotationType?.icon || '📝'}
                      </Avatar>
                    </ListItemAvatar>

                    <ListItemText
                      primary={
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                          <Typography variant='subtitle2' noWrap>
                            {annotation.title}
                          </Typography>
                          {annotation.is_pinned && <PinIcon sx={{ fontSize: 16 }} />}
                          {!annotation.is_visible && <VisibilityOffIcon sx={{ fontSize: 16 }} />}
                        </Box>
                      }
                      secondary={
                        <Box>
                          <Typography variant='caption' color='text.secondary'>
                            {author?.name || 'Loading...'} •{' '}
                            {annotation.created_at
                              ? format(new Date(annotation.created_at), 'MMM d, h:mm a')
                              : 'Just now'}
                          </Typography>
                          <br />
                          <Typography variant='body2' noWrap>
                            {annotation.description || annotation.title}
                          </Typography>
                          {annotation.tags && annotation.tags.length > 0 && (
                            <Box sx={{ mt: 0.5 }}>
                              {annotation.tags.map(tag => (
                                <Chip
                                  key={tag}
                                  label={tag}
                                  size='small'
                                  sx={{ mr: 0.5, fontSize: '0.7rem' }}
                                />
                              ))}
                            </Box>
                          )}
                          {commentCount > 0 && (
                            <Typography variant='caption' color='primary'>
                              {commentCount} comment{commentCount !== 1 ? 's' : ''}
                            </Typography>
                          )}
                        </Box>
                      }
                    />

                    <ListItemSecondaryAction>
                      <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
                        <IconButton
                          size='small'
                          onClick={e => {
                            e.stopPropagation();
                            toggleAnnotationVisibility(annotation.id);
                          }}
                        >
                          {annotation.is_visible ? <VisibilityIcon /> : <VisibilityOffIcon />}
                        </IconButton>

                        <IconButton
                          size='small'
                          onClick={e => {
                            e.stopPropagation();
                            toggleAnnotationPin(annotation.id);
                          }}
                          color={annotation.is_pinned ? 'primary' : 'default'}
                        >
                          <PinIcon />
                        </IconButton>

                        {annotation.user_id === currentUser.id && (
                          <IconButton
                            size='small'
                            onClick={e => {
                              e.stopPropagation();
                              handleDeleteAnnotation(annotation.id);
                            }}
                            color='error'
                            aria-label={`Delete annotation: ${annotation.title}`}
                          >
                            <DeleteIcon />
                          </IconButton>
                        )}
                      </Box>
                    </ListItemSecondaryAction>
                  </ListItem>
                );
              })}
            </List>

            {filteredAnnotations.length === 0 && !loading && (
              <Alert severity='info' sx={{ mt: 2 }}>
                No annotations found. Click "Add Annotation" to create your first annotation.
              </Alert>
            )}
          </Box>
        </Box>
      </Drawer>

      {/* New Annotation Dialog */}
      <Dialog
        open={newAnnotationDialog}
        onClose={() => setNewAnnotationDialog(false)}
        maxWidth='sm'
        fullWidth
      >
        <DialogTitle>Add Chart Annotation</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: 1 }}>
            <TextField
              label='Title'
              value={annotationForm.title}
              onChange={e => setAnnotationForm(prev => ({ ...prev, title: e.target.value }))}
              required
              fullWidth
              aria-label='Annotation title'
            />

            <TextField
              label='Content'
              value={annotationForm.content}
              onChange={e => setAnnotationForm(prev => ({ ...prev, content: e.target.value }))}
              multiline
              rows={3}
              required
              fullWidth
              aria-label='Annotation content'
            />

            <TextField
              label='Date'
              type='date'
              value={annotationForm.annotationDate}
              onChange={e =>
                setAnnotationForm(prev => ({ ...prev, annotationDate: e.target.value }))
              }
              required
              InputLabelProps={{ shrink: true }}
              fullWidth
              aria-label='Annotation date'
            />

            <TextField
              label='Value (optional)'
              type='number'
              value={annotationForm.annotationValue}
              onChange={e =>
                setAnnotationForm(prev => ({ ...prev, annotationValue: e.target.value }))
              }
              fullWidth
              aria-label='Annotation value (optional)'
            />

            <FormControl fullWidth>
              <InputLabel>Annotation Type</InputLabel>
              <Select
                value={annotationForm.annotationType}
                onChange={e =>
                  setAnnotationForm(prev => ({ ...prev, annotationType: e.target.value }))
                }
                label='Annotation Type'
                aria-label='Annotation type selection'
                MenuProps={{ disablePortal: true }}
              >
                {ANNOTATION_TYPES.map(type => (
                  <MenuItem key={type.value} value={type.value}>
                    {type.icon} {type.label}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>

            <Box>
              <Typography variant='subtitle2' sx={{ mb: 1 }}>
                Color
              </Typography>
              <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                {ANNOTATION_COLORS.map(color => (
                  <Box
                    key={color}
                    sx={{
                      width: 32,
                      height: 32,
                      bgcolor: color,
                      borderRadius: 1,
                      cursor: 'pointer',
                      border: annotationForm.color === color ? 3 : 1,
                      borderColor: annotationForm.color === color ? 'primary.main' : 'divider',
                    }}
                    onClick={() => setAnnotationForm(prev => ({ ...prev, color }))}
                  />
                ))}
              </Box>
            </Box>

            <FormControl fullWidth>
              <InputLabel>Visibility</InputLabel>
              <Select
                value={annotationForm.isPublic ? 'true' : 'false'}
                onChange={e =>
                  setAnnotationForm(prev => ({ ...prev, isPublic: e.target.value === 'true' }))
                }
                label='Visibility'
                inputProps={{
                  'aria-label': 'Select annotation visibility',
                }}
                MenuProps={{ disablePortal: true }}
              >
                <MenuItem value='true'>Public - Visible to all collaborators</MenuItem>
                <MenuItem value='false'>Private - Only visible to you</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setNewAnnotationDialog(false)}>Cancel</Button>
          <Button
            onClick={handleCreateAnnotation}
            variant='contained'
            disabled={loading}
            data-testid='submit-annotation-button'
          >
            Add Annotation
          </Button>
        </DialogActions>
      </Dialog>

      {/* Comments Dialog */}
      <Dialog
        open={!!selectedAnnotation}
        onClose={() => setSelectedAnnotation(null)}
        maxWidth='md'
        fullWidth
      >
        <DialogTitle>{selectedAnnotation?.title} - Comments</DialogTitle>
        <DialogContent>
          {selectedAnnotation && (
            <Box>
              <Typography variant='body2' color='text.secondary' sx={{ mb: 2 }}>
                {selectedAnnotation.description}
              </Typography>

              {(getCommentsForAnnotation(selectedAnnotation.id)?.length || 0) > 0 && (
                <List sx={{ mb: 2 }}>
                  {(getCommentsForAnnotation(selectedAnnotation.id) || []).map(comment => {
                    const author = getUserById(comment.user_id);
                    return (
                      <ListItem key={comment.id} alignItems='flex-start'>
                        <ListItemAvatar>
                          <Avatar src={author?.avatarUrl}>{author?.name?.[0] || '?'}</Avatar>
                        </ListItemAvatar>
                        <ListItemText
                          primary={author?.name || 'Loading...'}
                          secondary={
                            <Box>
                              <Box component='span' sx={{ display: 'block', mt: 0.5 }}>
                                {comment.content}
                              </Box>
                              <Box
                                component='span'
                                sx={{
                                  display: 'block',
                                  fontSize: '0.75rem',
                                  color: 'text.secondary',
                                }}
                              >
                                {comment.created_at
                                  ? format(new Date(comment.created_at), 'MMM d, h:mm a')
                                  : 'Just now'}
                              </Box>
                            </Box>
                          }
                        />
                      </ListItem>
                    );
                  })}
                </List>
              )}

              <Box sx={{ display: 'flex', gap: 1 }}>
                <TextField
                  label='Add a comment...'
                  value={newComment}
                  onChange={e => setNewComment(e.target.value)}
                  multiline
                  inputProps={{ 'data-testid': 'comment-input' }}
                  rows={2}
                  fullWidth
                />
                <Button
                  variant='contained'
                  onClick={handleAddComment}
                  disabled={!newComment.trim() || loading}
                >
                  Comment
                </Button>
              </Box>
            </Box>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setSelectedAnnotation(null)}>Close</Button>
        </DialogActions>
      </Dialog>

      {/* Share Chart Dialog */}
      <Dialog
        open={shareDialog}
        onClose={() => setShareDialog(false)}
        maxWidth='sm'
        fullWidth
        data-testid='share-chart-dialog'
      >
        <DialogTitle>Share Chart</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: 1 }}>
            <TextField
              label='User ID to share with'
              value={shareForm.targetUserId}
              onChange={e => setShareForm(prev => ({ ...prev, targetUserId: e.target.value }))}
              required
              fullWidth
              helperText='Enter the user ID of the person you want to share with'
              inputProps={{ 'data-testid': 'share-target-user-input' }}
              aria-label='Target user ID'
            />

            <FormControl fullWidth>
              <InputLabel>Permission Level</InputLabel>
              <Select
                value={shareForm.permissionLevel}
                onChange={e => setShareForm(prev => ({ ...prev, permissionLevel: e.target.value }))}
                label='Permission Level'
                inputProps={{ 'data-testid': 'share-permission-level-select' }}
                aria-label='Permission level'
                MenuProps={{ disablePortal: true }}
              >
                <MenuItem value='view'>View - Can view annotations</MenuItem>
                <MenuItem value='comment'>Comment - Can view and comment</MenuItem>
                <MenuItem value='edit'>Edit - Can create and edit annotations</MenuItem>
                <MenuItem value='admin'>Admin - Full access</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setShareDialog(false)}>
            Cancel
          </Button>
          <Button
            onClick={handleShareChart}
            variant='contained'
            disabled={loading}
            data-testid='submit-share-button'
          >
            Share Chart
          </Button>
        </DialogActions>
      </Dialog>

      {/* Snackbar for notifications */}
      <Snackbar
        open={snackbar.open}
        autoHideDuration={6000}
        onClose={() => setSnackbar(prev => ({ ...prev, open: false }))}
        message={snackbar.message}
      />
    </>
  );
};

export default ChartCollaborationConnected;
