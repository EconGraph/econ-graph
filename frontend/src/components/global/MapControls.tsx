/**
 * MapControls Component.
 *
 * Control panel for the interactive world map including zoom controls,
 * projection selector, and map settings.
 */

import React from 'react';
import {
  Box,
  Paper,
  IconButton,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Switch,
  FormControlLabel,
  Typography,
  Divider,
  Tooltip,
  Button,
} from '@mui/material';
import {
  ZoomIn,
  ZoomOut,
  Refresh,
  Public,
  Map,
  PublicOff,
  Visibility,
  VisibilityOff,
} from '@mui/icons-material';
import { ARIA_LABELS, keyboardHandlers } from './accessibility-utils';

interface MapControlsProps {
  /** Current zoom level */
  zoomLevel: number;
  /** Current projection type */
  projection: string;
  /** Available projections */
  projections: string[];
  /** Whether borders are shown */
  showBorders: boolean;
  /** Whether labels are shown */
  showLabels: boolean;
  /** Callback for zoom in */
  onZoomIn: () => void;
  /** Callback for zoom out */
  onZoomOut: () => void;
  /** Callback for reset zoom */
  onResetZoom: () => void;
  /** Callback for projection change */
  onProjectionChange: (projection: string) => void;
  /** Callback for border toggle */
  onBordersToggle: (show: boolean) => void;
  /** Callback for labels toggle */
  onLabelsToggle: (show: boolean) => void;
  /** Whether controls are disabled */
  disabled?: boolean;
}

const MapControls: React.FC<MapControlsProps> = ({
  zoomLevel,
  projection,
  projections,
  showBorders,
  showLabels,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onProjectionChange,
  onBordersToggle,
  onLabelsToggle,
  disabled = false,
}) => {
  const getProjectionIcon = (proj: string) => {
    switch (proj) {
      case 'naturalEarth':
        return <Public />;
      case 'mercator':
        return <Map />;
      case 'orthographic':
        return <PublicOff />;
      default:
        return <Public />;
    }
  };

  const getProjectionLabel = (proj: string) => {
    switch (proj) {
      case 'naturalEarth':
        return 'Natural Earth';
      case 'mercator':
        return 'Mercator';
      case 'orthographic':
        return 'Orthographic';
      default:
        return proj;
    }
  };

  return (
    <Paper
      elevation={3}
      sx={{
        p: 2,
        minWidth: 280,
        maxWidth: 320,
        bgcolor: 'background.paper',
        borderRadius: 2,
      }}
      role='region'
      aria-label={ARIA_LABELS.MAP_CONTAINER}
    >
      <Typography
        variant='h6'
        gutterBottom
        sx={{ display: 'flex', alignItems: 'center', gap: 1 }}
        id='map-controls-heading'
      >
        <Public />
        Map Controls
      </Typography>

      {/* Zoom Controls */}
      <Box sx={{ mb: 2 }}>
        <Typography variant='subtitle2' gutterBottom id='zoom-controls-label'>
          Zoom & Navigation
        </Typography>
        <Box
          sx={{ display: 'flex', gap: 1, alignItems: 'center' }}
          role='group'
          aria-labelledby='zoom-controls-label'
        >
          <Tooltip title='Zoom Out'>
            <IconButton
              onClick={onZoomOut}
              disabled={disabled}
              size='small'
              color='primary'
              aria-label={ARIA_LABELS.ZOOM_OUT}
              onKeyDown={e =>
                keyboardHandlers.handleActivationKey(e as unknown as KeyboardEvent, onZoomOut)
              }
            >
              <ZoomOut />
            </IconButton>
          </Tooltip>

          <Typography
            variant='body2'
            sx={{ minWidth: 60, textAlign: 'center' }}
            aria-live='polite'
            aria-label={`Current zoom level: ${Math.round(zoomLevel * 100)} percent`}
          >
            {Math.round(zoomLevel * 100)}%
          </Typography>

          <Tooltip title='Zoom In'>
            <IconButton
              onClick={onZoomIn}
              disabled={disabled}
              size='small'
              color='primary'
              aria-label={ARIA_LABELS.ZOOM_IN}
              onKeyDown={e =>
                keyboardHandlers.handleActivationKey(e as unknown as KeyboardEvent, onZoomIn)
              }
            >
              <ZoomIn />
            </IconButton>
          </Tooltip>

          <Tooltip title='Reset View'>
            <IconButton
              onClick={onResetZoom}
              disabled={disabled}
              size='small'
              color='secondary'
              aria-label={ARIA_LABELS.RESET_ZOOM}
              onKeyDown={e =>
                keyboardHandlers.handleActivationKey(e as unknown as KeyboardEvent, onResetZoom)
              }
            >
              <Refresh />
            </IconButton>
          </Tooltip>
        </Box>
      </Box>

      <Divider sx={{ my: 2 }} />

      {/* Projection Selector */}
      <Box sx={{ mb: 2 }}>
        <FormControl fullWidth size='small'>
          <InputLabel id='projection-label'>Map Projection</InputLabel>
          <Select
            value={projection}
            onChange={e => onProjectionChange(e.target.value)}
            disabled={disabled}
            label='Map Projection'
            labelId='projection-label'
            aria-label={ARIA_LABELS.PROJECTION_SELECTOR}
            aria-describedby='projection-description'
          >
            {projections.map(proj => (
              <MenuItem key={proj} value={proj}>
                <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                  {getProjectionIcon(proj)}
                  {getProjectionLabel(proj)}
                </Box>
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        <Typography
          id='projection-description'
          variant='caption'
          color='text.secondary'
          sx={{ mt: 0.5, display: 'block' }}
        >
          Choose how the world map is displayed
        </Typography>
      </Box>

      <Divider sx={{ my: 2 }} />

      {/* Display Options */}
      <Box sx={{ mb: 2 }}>
        <Typography variant='subtitle2' gutterBottom id='display-options-label'>
          Display Options
        </Typography>

        <FormControlLabel
          control={
            <Switch
              checked={showBorders}
              onChange={e => onBordersToggle(e.target.checked)}
              disabled={disabled}
              size='small'
              aria-label={ARIA_LABELS.BORDERS_TOGGLE}
              aria-describedby='borders-description'
            />
          }
          label={
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
              {showBorders ? <Visibility /> : <VisibilityOff />}
              <Typography variant='body2'>Show Borders</Typography>
            </Box>
          }
        />
        <Typography
          id='borders-description'
          variant='caption'
          color='text.secondary'
          sx={{ ml: 4, display: 'block' }}
        >
          {showBorders ? 'Country borders are visible' : 'Country borders are hidden'}
        </Typography>

        <FormControlLabel
          control={
            <Switch
              checked={showLabels}
              onChange={e => onLabelsToggle(e.target.checked)}
              disabled={disabled}
              size='small'
              aria-label={ARIA_LABELS.LABELS_TOGGLE}
              aria-describedby='labels-description'
            />
          }
          label={
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
              {showLabels ? <Visibility /> : <VisibilityOff />}
              <Typography variant='body2'>Show Labels</Typography>
            </Box>
          }
        />
        <Typography
          id='labels-description'
          variant='caption'
          color='text.secondary'
          sx={{ ml: 4, display: 'block' }}
        >
          {showLabels ? 'Country names are visible' : 'Country names are hidden'}
        </Typography>
      </Box>

      <Divider sx={{ my: 2 }} />

      {/* Quick Actions */}
      <Box>
        <Typography variant='subtitle2' gutterBottom id='quick-actions-label'>
          Quick Actions
        </Typography>
        <Button
          variant='outlined'
          size='small'
          fullWidth
          onClick={onResetZoom}
          disabled={disabled}
          startIcon={<Refresh />}
          aria-label={ARIA_LABELS.RESET_ZOOM}
          aria-describedby='reset-description'
        >
          Reset to Default View
        </Button>
        <Typography
          id='reset-description'
          variant='caption'
          color='text.secondary'
          sx={{ mt: 0.5, display: 'block' }}
        >
          Return map to original position and zoom level
        </Typography>
      </Box>
    </Paper>
  );
};

export default MapControls;
