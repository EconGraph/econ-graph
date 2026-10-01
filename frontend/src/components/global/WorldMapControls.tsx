/**
 * WorldMapControls Component.
 *
 * Projection and color scheme pickers for the world map. Zoom controls sit on the map itself.
 */

import React from 'react';
import { Box, FormControl, InputLabel, Select, MenuItem } from '@mui/material';
import { PROJECTIONS } from './hooks/useWorldMap';
import { COLOR_SCHEMES } from './hooks/useCountryData';

interface WorldMapControlsProps {
  /** A key of `PROJECTIONS`. */
  projection: string;
  onProjectionChange: (projection: string) => void;
  /** A key of `COLOR_SCHEMES`. */
  colorScheme: string;
  onColorSchemeChange: (colorScheme: string) => void;
}

const COLOR_SCHEME_NAMES: Record<keyof typeof COLOR_SCHEMES, string> = {
  viridis: 'Viridis',
  blues: 'Blues',
  reds: 'Reds',
  greens: 'Greens',
};

const WorldMapControls: React.FC<WorldMapControlsProps> = ({
  projection,
  onProjectionChange,
  colorScheme,
  onColorSchemeChange,
}) => (
  <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap' }}>
    <FormControl size='small' sx={{ minWidth: 160 }}>
      <InputLabel id='projection-select-label'>Projection</InputLabel>
      <Select
        labelId='projection-select-label'
        value={projection}
        label='Projection'
        onChange={event => onProjectionChange(event.target.value)}
      >
        {Object.entries(PROJECTIONS).map(([key, { name }]) => (
          <MenuItem key={key} value={key}>
            {name}
          </MenuItem>
        ))}
      </Select>
    </FormControl>

    <FormControl size='small' sx={{ minWidth: 140 }}>
      <InputLabel id='color-scheme-select-label'>Color scheme</InputLabel>
      <Select
        labelId='color-scheme-select-label'
        value={colorScheme}
        label='Color scheme'
        onChange={event => onColorSchemeChange(event.target.value)}
      >
        {Object.entries(COLOR_SCHEME_NAMES).map(([key, name]) => (
          <MenuItem key={key} value={key}>
            {name}
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  </Box>
);

export default WorldMapControls;
