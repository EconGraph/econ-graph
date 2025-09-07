/**
 * REQUIREMENT: Chart collaboration features with annotations and comments
 * PURPOSE: Enable real-time collaborative economic analysis with shared annotations
 * This provides Bloomberg Terminal-style collaboration for economic research
 */

import React, { useState, useCallback, useEffect } from 'react';
import {
  Box,
  Paper,
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
} from '@mui/material';
import {
  Add as AddIcon,
  Edit as EditIcon,
  Delete as DeleteIcon,
  Share as ShareIcon,
  Comment as CommentIcon,
  Timeline as TimelineIcon,
  Person as PersonIcon,
  Visibility as VisibilityIcon,
  VisibilityOff as VisibilityOffIcon,
  PushPin as PinIcon,
  Flag as FlagIcon,
} from '@mui/icons-material';
import { format } from 'date-fns';

export interface ChartAnnotation {
  id: string;
  date: string;
  value?: number;
  title: string;
  description: string;
  color: string;
  type: 'line' | 'point' | 'box' | 'trend';
  author: {
    id: string;
    name: string;
    avatar?: string;
  };
  createdAt: string;
  updatedAt: string;
  isVisible: boolean;
  isPinned: boolean;
  tags: string[];
  comments: ChartComment[];
}

export interface ChartComment {
  id: string;
  content: string;
  author: {
    id: string;
    name: string;
    avatar?: string;
  };
  createdAt: string;
  isResolved: boolean;
}

interface ChartCollaborationProps {
  annotations: ChartAnnotation[];
  onAnnotationAdd: (annotation: Omit<ChartAnnotation, 'id' | 'createdAt' | 'updatedAt'>) => void;
  onAnnotationUpdate: (id: string, updates: Partial<ChartAnnotation>) => void;
  onAnnotationDelete: (id: string) => void;
  onCommentAdd: (annotationId: string, comment: Omit<ChartComment, 'id' | 'createdAt'>) => void;
  currentUser: {
    id: string;
    name: string;
    avatar?: string;
  };
  collaborators: Array<{
    id: string;
    name: string;
    avatar?: string;
    isOnline: boolean;
    role: 'owner' | 'editor' | 'viewer';
  }>;
  isOpen: boolean;
  onToggle: () => void;
}

const ANNOTATION_COLORS = [
  '#f44336', '#e91e63', '#9c27b0', '#673ab7',
  '#3f51b5', '#2196f3', '#03a9f4', '#00bcd4',
  '#009688', '#4caf50', '#8bc34a', '#cddc39',
  '#ffeb3b', '#ffc107', '#ff9800', '#ff5722'
];

const ANNOTATION_TYPES = [
  { value: 'line', label: 'Vertical Line', icon: '│' },
  { value: 'point', label: 'Data Point', icon: '●' },
  { value: 'box', label: 'Range Box', icon: '▢' },
  { value: 'trend', label: 'Trend Line', icon: '⟋' },
];

const ChartCollaboration: React.FC<ChartCollaborationProps> = ({
  annotations,
  onAnnotationAdd,
  onAnnotationUpdate,
  onAnnotationDelete,
  onCommentAdd,
  currentUser,
  collaborators,
  isOpen,
  onToggle,
}) => {
  const [newAnnotationDialog, setNewAnnotationDialog] = useState(false);
  const [editingAnnotation, setEditingAnnotation] = useState<ChartAnnotation | null>(null);
  const [selectedAnnotation, setSelectedAnnotation] = useState<ChartAnnotation | null>(null);
  const [newComment, setNewComment] = useState('');
  const [filterBy, setFilterBy] = useState<'all' | 'mine' | 'pinned'>('all');

  // New annotation form state
  const [annotationForm, setAnnotationForm] = useState({
    date: '',
    value: '',
    title: '',
    description: '',
    color: ANNOTATION_COLORS[0],
    type: 'line' as ChartAnnotation['type'],
    tags: '',
  });

  const resetAnnotationForm = useCallback(() => {
    setAnnotationForm({
      date: '',
      value: '',
      title: '',
      description: '',
      color: ANNOTATION_COLORS[0],
      type: 'line',
      tags: '',
    });
  }, []);

  const handleCreateAnnotation = useCallback(() => {
    if (!annotationForm.title || !annotationForm.date) return;

    const newAnnotation: Omit<ChartAnnotation, 'id' | 'createdAt' | 'updatedAt'> = {
      date: annotationForm.date,
      value: annotationForm.value ? parseFloat(annotationForm.value) : undefined,
      title: annotationForm.title,
      description: annotationForm.description,
      color: annotationForm.color,
      type: annotationForm.type,
      author: currentUser,
      isVisible: true,
      isPinned: false,
      tags: annotationForm.tags.split(',').map(tag => tag.trim()).filter(Boolean),
      comments: [],
    };

    onAnnotationAdd(newAnnotation);
    setNewAnnotationDialog(false);
    resetAnnotationForm();
  }, [annotationForm, currentUser, onAnnotationAdd, resetAnnotationForm]);

  const handleUpdateAnnotation = useCallback((id: string, updates: Partial<ChartAnnotation>) => {
    onAnnotationUpdate(id, updates);
    setEditingAnnotation(null);
  }, [onAnnotationUpdate]);

  const handleAddComment = useCallback(() => {
    if (!selectedAnnotation || !newComment.trim()) return;

    const comment: Omit<ChartComment, 'id' | 'createdAt'> = {
      content: newComment,
      author: currentUser,
      isResolved: false,
    };

    onCommentAdd(selectedAnnotation.id, comment);
    setNewComment('');
  }, [selectedAnnotation, newComment, currentUser, onCommentAdd]);

  const filteredAnnotations = annotations.filter(annotation => {
    switch (filterBy) {
      case 'mine':
        return annotation.author.id === currentUser.id;
      case 'pinned':
        return annotation.isPinned;
      default:
        return true;
    }
  });

  const visibleAnnotations = filteredAnnotations.filter(a => a.isVisible);
  const totalComments = annotations.reduce((sum, a) => sum + a.comments.length, 0);

  return (
    <>
      {/* Collaboration Panel */}
      <Drawer
        anchor="right"
        open={isOpen}
        onClose={onToggle}
        variant="persistent"
        sx={{
          width: 400,
          flexShrink: 0,
          '& .MuiDrawer-paper': {
            width: 400,
            boxSizing: 'border-box',
            top: 64, // Account for app bar
            height: 'calc(100% - 64px)',
          },
        }}
      >
        <Box sx={{ p: 2 }}>
          {/* Header */}
          <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
            <Typography variant="h6" sx={{ flexGrow: 1 }}>
              Chart Collaboration
            </Typography>
            <Badge badgeContent={totalComments} color="primary">
              <CommentIcon />
            </Badge>
          </Box>

          {/* Collaborators */}
          <Box sx={{ mb: 3 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              Active Collaborators ({collaborators.filter(c => c.isOnline).length})
            </Typography>
            <Box sx={{ display: 'flex', gap: 1 }}>
              {collaborators.slice(0, 5).map((collaborator) => (
                <Tooltip key={collaborator.id} title={`${collaborator.name} (${collaborator.role})`}>
                  <Badge
                    color={collaborator.isOnline ? 'success' : 'default'}
                    variant="dot"
                    anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
                  >
                    <Avatar 
                      src={collaborator.avatar}
                      sx={{ width: 32, height: 32 }}
                    >
                      {collaborator.name[0]}
                    </Avatar>
                  </Badge>
                </Tooltip>
              ))}
              {collaborators.length > 5 && (
                <Avatar sx={{ width: 32, height: 32 }}>
                  +{collaborators.length - 5}
                </Avatar>
              )}
            </Box>
          </Box>

          <Divider sx={{ mb: 2 }} />

          {/* Controls */}
          <Box sx={{ display: 'flex', gap: 1, mb: 2 }}>
            <Button
              variant="contained"
              startIcon={<AddIcon />}
              onClick={() => setNewAnnotationDialog(true)}
              size="small"
              fullWidth
            >
              Add Annotation
            </Button>
          </Box>

          {/* Filter */}
          <FormControl size="small" fullWidth sx={{ mb: 2 }}>
            <InputLabel>Filter Annotations</InputLabel>
            <Select
              value={filterBy}
              onChange={(e) => setFilterBy(e.target.value as any)}
              label="Filter Annotations"
            >
              <MenuItem value="all">All Annotations ({annotations.length})</MenuItem>
              <MenuItem value="mine">My Annotations ({annotations.filter(a => a.author.id === currentUser.id).length})</MenuItem>
              <MenuItem value="pinned">Pinned ({annotations.filter(a => a.isPinned).length})</MenuItem>
            </Select>
          </FormControl>

          {/* Annotations List */}
          <Typography variant="subtitle2" sx={{ mb: 1 }}>
            Annotations ({visibleAnnotations.length})
          </Typography>
          
          <List sx={{ maxHeight: 400, overflow: 'auto' }}>
            {visibleAnnotations.map((annotation) => (
              <ListItem
                key={annotation.id}
                sx={{
                  border: 1,
                  borderColor: 'divider',
                  borderRadius: 1,
                  mb: 1,
                  bgcolor: annotation.isPinned ? 'action.hover' : 'background.paper',
                }}
              >
                <ListItemAvatar>
                  <Avatar
                    src={annotation.author.avatar}
                    sx={{ 
                      bgcolor: annotation.color,
                      width: 32,
                      height: 32,
                    }}
                  >
                    {ANNOTATION_TYPES.find(t => t.value === annotation.type)?.icon || '●'}
                  </Avatar>
                </ListItemAvatar>
                
                <ListItemText
                  primary={
                    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                      <Typography variant="subtitle2" noWrap>
                        {annotation.title}
                      </Typography>
                      {annotation.isPinned && <PinIcon sx={{ fontSize: 16 }} />}
                    </Box>
                  }
                  secondary={
                    <Box>
                      <Typography variant="caption" color="text.secondary">
                        {annotation.author.name} • {format(new Date(annotation.createdAt), 'MMM d, h:mm a')}
                      </Typography>
                      <br />
                      <Typography variant="body2" noWrap>
                        {annotation.description}
                      </Typography>
                      {annotation.tags.length > 0 && (
                        <Box sx={{ mt: 0.5 }}>
                          {annotation.tags.map((tag) => (
                            <Chip key={tag} label={tag} size="small" sx={{ mr: 0.5, fontSize: '0.7rem' }} />
                          ))}
                        </Box>
                      )}
                      {annotation.comments.length > 0 && (
                        <Typography variant="caption" color="primary">
                          {annotation.comments.length} comment{annotation.comments.length !== 1 ? 's' : ''}
                        </Typography>
                      )}
                    </Box>
                  }
                />
                
                <ListItemSecondaryAction>
                  <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.5 }}>
                    <IconButton
                      size="small"
                      onClick={() => handleUpdateAnnotation(annotation.id, { isVisible: !annotation.isVisible })}
                    >
                      {annotation.isVisible ? <VisibilityIcon /> : <VisibilityOffIcon />}
                    </IconButton>
                    
                    <IconButton
                      size="small"
                      onClick={() => handleUpdateAnnotation(annotation.id, { isPinned: !annotation.isPinned })}
                      color={annotation.isPinned ? 'primary' : 'default'}
                    >
                      <PinIcon />
                    </IconButton>
                    
                    <IconButton
                      size="small"
                      onClick={() => setSelectedAnnotation(annotation)}
                    >
                      <CommentIcon />
                    </IconButton>
                    
                    {annotation.author.id === currentUser.id && (
                      <IconButton
                        size="small"
                        onClick={() => onAnnotationDelete(annotation.id)}
                        color="error"
                      >
                        <DeleteIcon />
                      </IconButton>
                    )}
                  </Box>
                </ListItemSecondaryAction>
              </ListItem>
            ))}
          </List>

          {visibleAnnotations.length === 0 && (
            <Alert severity="info" sx={{ mt: 2 }}>
              No annotations found. Click "Add Annotation" to create your first annotation.
            </Alert>
          )}
        </Box>
      </Drawer>

      {/* New Annotation Dialog */}
      <Dialog
        open={newAnnotationDialog}
        onClose={() => setNewAnnotationDialog(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>Add Chart Annotation</DialogTitle>
        <DialogContent>
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: 1 }}>
            <TextField
              label="Title"
              value={annotationForm.title}
              onChange={(e) => setAnnotationForm(prev => ({ ...prev, title: e.target.value }))}
              required
              fullWidth
            />
            
            <TextField
              label="Description"
              value={annotationForm.description}
              onChange={(e) => setAnnotationForm(prev => ({ ...prev, description: e.target.value }))}
              multiline
              rows={3}
              fullWidth
            />
            
            <TextField
              label="Date (YYYY-MM-DD)"
              type="date"
              value={annotationForm.date}
              onChange={(e) => setAnnotationForm(prev => ({ ...prev, date: e.target.value }))}
              required
              InputLabelProps={{ shrink: true }}
              fullWidth
            />
            
            <TextField
              label="Value (optional)"
              type="number"
              value={annotationForm.value}
              onChange={(e) => setAnnotationForm(prev => ({ ...prev, value: e.target.value }))}
              fullWidth
            />
            
            <FormControl fullWidth>
              <InputLabel>Annotation Type</InputLabel>
              <Select
                value={annotationForm.type}
                onChange={(e) => setAnnotationForm(prev => ({ ...prev, type: e.target.value as any }))}
                label="Annotation Type"
              >
                {ANNOTATION_TYPES.map((type) => (
                  <MenuItem key={type.value} value={type.value}>
                    {type.icon} {type.label}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>
            
            <Box>
              <Typography variant="subtitle2" sx={{ mb: 1 }}>Color</Typography>
              <Box sx={{ display: 'flex', gap: 1, flexWrap: 'wrap' }}>
                {ANNOTATION_COLORS.map((color) => (
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
            
            <TextField
              label="Tags (comma-separated)"
              value={annotationForm.tags}
              onChange={(e) => setAnnotationForm(prev => ({ ...prev, tags: e.target.value }))}
              placeholder="analysis, trend, important"
              fullWidth
            />
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setNewAnnotationDialog(false)}>Cancel</Button>
          <Button onClick={handleCreateAnnotation} variant="contained">
            Add Annotation
          </Button>
        </DialogActions>
      </Dialog>

      {/* Comments Dialog */}
      <Dialog
        open={!!selectedAnnotation}
        onClose={() => setSelectedAnnotation(null)}
        maxWidth="md"
        fullWidth
      >
        <DialogTitle>
          {selectedAnnotation?.title} - Comments
        </DialogTitle>
        <DialogContent>
          {selectedAnnotation && (
            <Box>
              <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                {selectedAnnotation.description}
              </Typography>
              
              {selectedAnnotation.comments.length > 0 && (
                <List sx={{ mb: 2 }}>
                  {selectedAnnotation.comments.map((comment) => (
                    <ListItem key={comment.id} alignItems="flex-start">
                      <ListItemAvatar>
                        <Avatar src={comment.author.avatar}>
                          {comment.author.name[0]}
                        </Avatar>
                      </ListItemAvatar>
                      <ListItemText
                        primary={comment.author.name}
                        secondary={
                          <Box>
                            <Typography variant="body2" sx={{ mt: 0.5 }}>
                              {comment.content}
                            </Typography>
                            <Typography variant="caption" color="text.secondary">
                              {format(new Date(comment.createdAt), 'MMM d, h:mm a')}
                            </Typography>
                          </Box>
                        }
                      />
                    </ListItem>
                  ))}
                </List>
              )}
              
              <Box sx={{ display: 'flex', gap: 1 }}>
                <TextField
                  label="Add a comment..."
                  value={newComment}
                  onChange={(e) => setNewComment(e.target.value)}
                  multiline
                  rows={2}
                  fullWidth
                />
                <Button
                  variant="contained"
                  onClick={handleAddComment}
                  disabled={!newComment.trim()}
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
    </>
  );
};

export default ChartCollaboration;
