import React, { Suspense, useCallback, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Box,
  Typography,
  IconButton,
  Badge,
  Avatar,
  Tooltip,
  Button,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Divider,
  CircularProgress,
  Alert,
  Drawer,
  List,
  ListItem,
  ListItemAvatar,
  ListItemText,
  ListItemSecondaryAction,
  Chip,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  Snackbar,
} from '@mui/material';
import {
  Comment as CommentIcon,
  Close as CloseIcon,
  Add as AddIcon,
  Share as ShareIcon,
  PushPin as PinIcon,
  Visibility as VisibilityIcon,
  VisibilityOff as VisibilityOffIcon,
  Delete as DeleteIcon,
} from '@mui/icons-material';
import { useAuth } from '../../contexts/AuthContext';
import { executeGraphQL, QUERIES } from '../../utils/graphql';
import { format } from 'date-fns';

// Types
interface Collaborator {
  id: string;
  user_id: string;
  role: string;
  last_accessed_at: string;
}

interface User {
  id: string;
  name: string;
  email: string;
  avatarUrl?: string;
}

interface Annotation {
  id: string;
  title: string;
  description: string;
  content: string;
  user_id: string;
  created_at: string;
  is_pinned: boolean;
  is_visible: boolean;
  type?: string;
  comments?: Comment[];
}

interface Comment {
  id: string;
  content: string;
  user_id: string;
  created_at: string;
}

// GraphQL response types
interface ChartCollaboratorsResponse {
  chartCollaborators: Collaborator[];
}

interface UserResponse {
  user: User | null;
}

interface AnnotationsResponse {
  annotationsForChart: Annotation[];
}

interface CommentsResponse {
  commentsForAnnotation: Comment[];
}

// Data fetching functions using existing GraphQL setup
const fetchCollaborators = async (chartId: string): Promise<Collaborator[]> => {
  const response = await executeGraphQL<ChartCollaboratorsResponse>({
    query: QUERIES.GET_CHART_COLLABORATORS,
    variables: { chartId },
  });
  return response.data?.chartCollaborators || [];
};

const fetchUsers = async (userIds: string[]): Promise<Record<string, User>> => {
  if (userIds.length === 0) return {};

  // Fetch users individually since we don't have a batch query
  const userPromises = userIds.map(userId =>
    executeGraphQL<UserResponse>({
      query: QUERIES.GET_USER,
      variables: { userId },
    })
  );

  const responses = await Promise.all(userPromises);
  const users = responses.map(response => response.data?.user).filter(Boolean) as User[];

  return users.reduce(
    (acc, user) => {
      acc[user.id] = user;
      return acc;
    },
    {} as Record<string, User>
  );
};

const fetchAnnotations = async (chartId: string): Promise<Annotation[]> => {
  const response = await executeGraphQL<AnnotationsResponse>({
    query: QUERIES.GET_ANNOTATIONS,
    variables: { chartId },
  });
  return response.data?.annotationsForChart || [];
};

const fetchComments = async (annotationId: string): Promise<Comment[]> => {
  const response = await executeGraphQL<CommentsResponse>({
    query: QUERIES.GET_COMMENTS_FOR_ANNOTATION,
    variables: { annotationId },
  });
  return response.data?.commentsForAnnotation || [];
};

// React Query hooks with Suspense
const useCollaborators = (chartId: string) => {
  return useQuery({
    queryKey: ['collaborators', chartId],
    queryFn: () => fetchCollaborators(chartId),
    suspense: true,
    enabled: !!chartId,
  });
};

const useUsers = (userIds: string[]) => {
  return useQuery({
    queryKey: ['users', userIds.sort()],
    queryFn: () => fetchUsers(userIds),
    suspense: true,
    enabled: userIds.length > 0,
  });
};

const useAnnotations = (chartId: string) => {
  return useQuery({
    queryKey: ['annotations', chartId],
    queryFn: () => fetchAnnotations(chartId),
    suspense: true,
    enabled: !!chartId,
  });
};

const useComments = (annotationId: string) => {
  return useQuery({
    queryKey: ['comments', annotationId],
    queryFn: () => fetchComments(annotationId),
    suspense: true,
    enabled: !!annotationId,
  });
};

// Main content component that assumes all data is available
const ChartCollaborationContent = ({
  chartId,
  isOpen,
  onToggle,
}: {
  chartId: string;
  isOpen: boolean;
  onToggle: () => void;
}) => {
  const { user: currentUser } = useAuth();
  const queryClient = useQueryClient();
  const [filterBy, setFilterBy] = useState<'all' | 'mine' | 'pinned'>('all');
  const [selectedAnnotation, setSelectedAnnotation] = useState<Annotation | null>(null);
  const [newAnnotationDialog, setNewAnnotationDialog] = useState(false);
  const [shareDialog, setShareDialog] = useState(false);
  const [snackbar, setSnackbar] = useState({
    open: false,
    message: '',
    severity: 'success' as 'success' | 'error' | 'info' | 'warning',
  });

  // These hooks will suspend until data is available
  const { data: collaborators } = useCollaborators(chartId);
  const { data: annotations } = useAnnotations(chartId);

  // Get user IDs from collaborators
  const userIds = collaborators?.map(c => c.user_id) || [];
  const { data: users } = useUsers(userIds);

  // Calculate derived state (no loading states needed!)
  const activeCollaborators =
    collaborators?.filter(
      c => users?.[c.user_id] && Date.now() - new Date(c.last_accessed_at || 0).getTime() < 300000
    ) || [];

  const filteredAnnotations =
    annotations?.filter(annotation => {
      switch (filterBy) {
        case 'mine':
          return annotation.user_id === currentUser?.id;
        case 'pinned':
          return annotation.is_pinned;
        default:
          return annotation.is_visible !== false;
      }
    }) || [];

  const totalComments =
    annotations?.reduce((sum, annotation) => {
      return sum + (annotation.comments?.length || 0);
    }, 0) || 0;

  const showSnackbar = useCallback(
    (message: string, severity: 'success' | 'error' | 'info' | 'warning' = 'success') => {
      setSnackbar({ open: true, message, severity });
    },
    []
  );

  const handleCloseSnackbar = useCallback(() => {
    setSnackbar(prev => ({ ...prev, open: false }));
  }, []);

  // Mutations for actions
  const addAnnotationMutation = useMutation({
    mutationFn: async (newAnnotation: Partial<Annotation>) => {
      // Your mutation logic here
      return newAnnotation;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['annotations', chartId] });
      showSnackbar('Annotation added successfully');
      setNewAnnotationDialog(false);
    },
    onError: error => {
      showSnackbar(error instanceof Error ? error.message : 'Failed to add annotation', 'error');
    },
  });

  const shareChartMutation = useMutation({
    mutationFn: async ({ email, permission }: { email: string; permission: string }) => {
      // Your mutation logic here
      return { email, permission, success: true };
    },
    onSuccess: () => {
      showSnackbar('Chart shared successfully');
      setShareDialog(false);
    },
    onError: error => {
      showSnackbar(error instanceof Error ? error.message : 'Failed to share chart', 'error');
    },
  });

  return (
    <>
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
          <Box sx={{ mb: 3 }}>
            <Box
              sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 1 }}
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
                const user = users?.[collaborator.user_id];
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
            >
              <MenuItem value='all'>All Annotations ({annotations?.length || 0})</MenuItem>
              <MenuItem value='mine'>
                My Annotations (
                {annotations?.filter(a => a.user_id === currentUser?.id).length || 0})
              </MenuItem>
              <MenuItem value='pinned'>
                Pinned ({annotations?.filter(a => a.is_pinned).length || 0})
              </MenuItem>
            </Select>
          </FormControl>

          {/* Annotations List */}
          <Typography variant='subtitle2' sx={{ mb: 1 }}>
            Annotations ({filteredAnnotations.length})
          </Typography>
          <Box sx={{ flexGrow: 1, overflow: 'auto' }}>
            {filteredAnnotations.length === 0 ? (
              <Typography variant='body2' color='text.secondary'>
                No annotations found. Click "Add Annotation" to create one.
              </Typography>
            ) : (
              <List dense>
                {filteredAnnotations.map(annotation => (
                  <ListItem
                    key={annotation.id}
                    onClick={() => setSelectedAnnotation(annotation)}
                    sx={{
                      cursor: 'pointer',
                      '&:hover': { bgcolor: 'action.hover' },
                      borderRadius: 1,
                      mb: 1,
                    }}
                  >
                    <ListItemAvatar>
                      <Avatar src={users?.[annotation.user_id]?.avatarUrl}>
                        {users?.[annotation.user_id]?.name?.[0] || '?'}
                      </Avatar>
                    </ListItemAvatar>
                    <ListItemText
                      primary={
                        <Box sx={{ display: 'flex', alignItems: 'center' }}>
                          <Typography variant='body2' sx={{ fontWeight: 'bold', mr: 1 }}>
                            {annotation.title}
                          </Typography>
                          {annotation.type && (
                            <Chip label={annotation.type} size='small' sx={{ mr: 1 }} />
                          )}
                          {annotation.is_pinned && (
                            <Tooltip title='Pinned'>
                              <PinIcon fontSize='small' color='action' sx={{ mr: 0.5 }} />
                            </Tooltip>
                          )}
                          {annotation.is_visible === false && (
                            <Tooltip title='Hidden'>
                              <VisibilityOffIcon fontSize='small' color='action' sx={{ mr: 0.5 }} />
                            </Tooltip>
                          )}
                        </Box>
                      }
                      secondary={
                        <React.Fragment>
                          <Typography
                            sx={{ display: 'inline' }}
                            component='span'
                            variant='body2'
                            color='text.primary'
                          >
                            {users?.[annotation.user_id]?.name || 'Unknown User'}
                          </Typography>
                          <Typography component='div' variant='caption' color='textSecondary'>
                            {format(new Date(annotation.created_at), 'MMM dd, hh:mm a')}
                          </Typography>
                          <Typography component='div' variant='body2' color='textSecondary' noWrap>
                            {annotation.description}
                          </Typography>
                        </React.Fragment>
                      }
                    />
                    <ListItemSecondaryAction>
                      <IconButton
                        edge='end'
                        aria-label='toggle visibility'
                        onClick={e => {
                          e.stopPropagation();
                          // Toggle visibility logic
                        }}
                      >
                        {annotation.is_visible ? <VisibilityIcon /> : <VisibilityOffIcon />}
                      </IconButton>
                      <IconButton
                        edge='end'
                        aria-label='toggle pin'
                        onClick={e => {
                          e.stopPropagation();
                          // Toggle pin logic
                        }}
                      >
                        <PinIcon color={annotation.is_pinned ? 'primary' : 'action'} />
                      </IconButton>
                      <IconButton
                        edge='end'
                        aria-label='delete'
                        onClick={e => {
                          e.stopPropagation();
                          // Delete logic
                        }}
                        data-testid={`delete-annotation-${annotation.id}`}
                      >
                        <DeleteIcon />
                      </IconButton>
                    </ListItemSecondaryAction>
                  </ListItem>
                ))}
              </List>
            )}
          </Box>
        </Box>
      </Drawer>

      {/* New Annotation Dialog */}
      <Dialog open={newAnnotationDialog} onClose={() => setNewAnnotationDialog(false)}>
        <DialogTitle>Add New Annotation</DialogTitle>
        <DialogContent>
          <TextField
            margin='dense'
            id='annotation-title'
            label='Title'
            type='text'
            fullWidth
            variant='outlined'
            sx={{ mb: 2 }}
          />
          <TextField
            margin='dense'
            id='annotation-description'
            label='Description'
            type='text'
            fullWidth
            multiline
            rows={3}
            variant='outlined'
            sx={{ mb: 2 }}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setNewAnnotationDialog(false)}>Cancel</Button>
          <Button
            onClick={() => addAnnotationMutation.mutate({})}
            variant='contained'
            disabled={addAnnotationMutation.isPending}
          >
            Add
          </Button>
        </DialogActions>
      </Dialog>

      {/* Share Dialog */}
      <Dialog open={shareDialog} onClose={() => setShareDialog(false)}>
        <DialogTitle>Share Chart</DialogTitle>
        <DialogContent>
          <TextField
            margin='dense'
            id='share-email'
            label='User Email'
            type='email'
            fullWidth
            variant='outlined'
            sx={{ mb: 2 }}
          />
          <FormControl fullWidth margin='dense' sx={{ mb: 2 }}>
            <InputLabel id='share-permission-label'>Permission</InputLabel>
            <Select
              labelId='share-permission-label'
              id='share-permission'
              value='view'
              label='Permission'
            >
              <MenuItem value='view'>View</MenuItem>
              <MenuItem value='annotate'>Annotate</MenuItem>
              <MenuItem value='edit'>Edit</MenuItem>
            </Select>
          </FormControl>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setShareDialog(false)}>Cancel</Button>
          <Button
            onClick={() => shareChartMutation.mutate({ email: '', permission: 'view' })}
            variant='contained'
            disabled={shareChartMutation.isPending}
          >
            Share
          </Button>
        </DialogActions>
      </Dialog>

      {/* Comments Dialog */}
      <Dialog
        open={!!selectedAnnotation}
        onClose={() => setSelectedAnnotation(null)}
        fullWidth
        maxWidth='sm'
      >
        <DialogTitle>
          Comments for "{selectedAnnotation?.title}"
          <IconButton
            aria-label='close'
            onClick={() => setSelectedAnnotation(null)}
            sx={{
              position: 'absolute',
              right: 8,
              top: 8,
              color: theme => theme.palette.grey[500],
            }}
          >
            <CloseIcon />
          </IconButton>
        </DialogTitle>
        <DialogContent dividers>
          {selectedAnnotation && <CommentsList annotationId={selectedAnnotation.id} />}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setSelectedAnnotation(null)}>Close</Button>
        </DialogActions>
      </Dialog>

      {/* Snackbar for notifications */}
      <Snackbar
        open={snackbar.open}
        autoHideDuration={6000}
        onClose={handleCloseSnackbar}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'left' }}
      >
        <Alert onClose={handleCloseSnackbar} severity={snackbar.severity} sx={{ width: '100%' }}>
          {snackbar.message}
        </Alert>
      </Snackbar>
    </>
  );
};

// Comments component that also uses Suspense
const CommentsList = ({ annotationId }: { annotationId: string }) => {
  const { data: comments } = useComments(annotationId);
  const { data: users } = useUsers(comments?.map(c => c.user_id) || []);

  return (
    <List>
      {comments?.map(comment => (
        <ListItem key={comment.id} alignItems='flex-start'>
          <ListItemAvatar>
            <Avatar src={users?.[comment.user_id]?.avatarUrl}>
              {users?.[comment.user_id]?.name?.[0] || '?'}
            </Avatar>
          </ListItemAvatar>
          <ListItemText
            primary={
              <React.Fragment>
                <Typography
                  sx={{ display: 'inline' }}
                  component='span'
                  variant='body2'
                  color='text.primary'
                >
                  {users?.[comment.user_id]?.name || 'Unknown User'}
                </Typography>
                <Typography component='span' variant='caption' color='textSecondary' sx={{ ml: 1 }}>
                  {format(new Date(comment.created_at), 'MMM dd, hh:mm a')}
                </Typography>
              </React.Fragment>
            }
            secondary={
              <Typography variant='body2' color='textSecondary'>
                {comment.content}
              </Typography>
            }
          />
        </ListItem>
      ))}
    </List>
  );
};

// Main component with Suspense boundary
export const ChartCollaborationConnectedQuery = ({
  chartId,
  isOpen,
  onToggle,
}: {
  chartId: string;
  isOpen: boolean;
  onToggle: () => void;
}) => {
  const { user: currentUser } = useAuth();

  if (!currentUser) {
    return (
      <Box sx={{ p: 2, textAlign: 'center' }}>
        <Typography variant='body1' color='text.secondary'>
          Please log in to access collaboration features.
        </Typography>
      </Box>
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
      <ChartCollaborationContent chartId={chartId} isOpen={isOpen} onToggle={onToggle} />
    </Suspense>
  );
};
