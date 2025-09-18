/**
 * WorldMapControls Component
 *
 * Control panel for the interactive world map including zoom controls,
 * projection selection, and color scheme options.
 */

import React from 'react';
import {
  Box,
  Button,
  ButtonGroup,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Typography,
} from '@mui/material';
import { Add, Remove, Public, Explore, TravelExplore } from '@mui/icons-material';
import { useGlobalAnalysis } from '../../contexts/GlobalAnalysisContext';
import { ProjectionType, ColorScheme } from '../../types/globalAnalysis';

const WorldMapControls: React.FC = () => {
  const { state, actions } = useGlobalAnalysis();

  const handleZoom = (factor: number) => {
    actions.setMapView({
      ...state.mapView,
      scale: state.mapView.scale * factor,
    });
  };

  const handleProjectionChange = (event: any) => {
    actions.setProjection(event.target.value);
  };

  const handleColorSchemeChange = (event: any) => {
    actions.setColorScheme(event.target.value);
  };

  return (
    <Box
      sx={{
        display: 'flex',
        flexDirection: 'column',
        gap: 2,
        p: 2,
        border: '1px solid #e0e0e0',
        borderRadius: 2,
      }}
    >
      <Typography variant='h6' gutterBottom>
        Map Controls
      </Typography>

      {/* Zoom Controls */}
      <Box>
        <Typography variant='subtitle2' gutterBottom>
          Zoom
        </Typography>
        <ButtonGroup variant='outlined' aria-label='zoom controls'>
          <Button onClick={() => handleZoom(1.2)}>
            <Add />
          </Button>
          <Button onClick={() => handleZoom(0.8)}>
            <Remove />
          </Button>
        </ButtonGroup>
      </Box>

      {/* Projection Selector */}
      <FormControl fullWidth size='small'>
        <InputLabel id='projection-select-label'>Projection</InputLabel>
        <Select
          labelId='projection-select-label'
          value={state.projection}
          label='Projection'
          onChange={handleProjectionChange}
        >
          <MenuItem value='geoNaturalEarth1'>
            <Public sx={{ mr: 1 }} /> Natural Earth
          </MenuItem>
          <MenuItem value='geoMercator'>
            <Explore sx={{ mr: 1 }} /> Mercator
          </MenuItem>
          <MenuItem value='geoOrthographic'>
            <TravelExplore sx={{ mr: 1 }} /> Orthographic
          </MenuItem>
        </Select>
      </FormControl>

      {/* Color Scheme Selector */}
      <FormControl fullWidth size='small'>
        <InputLabel id='color-scheme-select-label'>Color Scheme</InputLabel>
        <Select
          labelId='color-scheme-select-label'
          value={state.colorScheme}
          label='Color Scheme'
          onChange={handleColorSchemeChange}
        >
          <MenuItem value='viridis'>Viridis</MenuItem>
          <MenuItem value='blues'>Blues</MenuItem>
          <MenuItem value='reds'>Reds</MenuItem>
        </Select>
      </FormControl>
    </Box>
  );
};

export default WorldMapControls;
