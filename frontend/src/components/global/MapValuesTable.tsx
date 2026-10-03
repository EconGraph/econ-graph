/**
 * MapValuesTable Component.
 *
 * The map's values as a table, collapsed under the map: every country with a value, highest
 * first, each linking to its series. It reaches what the map can't: keyboard and screen reader
 * users, and countries with no shape on the map, such as Kosovo.
 */

import React from 'react';
import { Link as RouterLink } from 'react-router-dom';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Link,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { ExpandMore } from '@mui/icons-material';
import type { MapCountryValue } from './hooks/useWorldMapData';
import { formatMapDate, formatMapValue } from './mapFormat';

interface MapValuesTableProps {
  values: MapCountryValue[];
  unit: string | null;
  frequency: string | null;
}

const MapValuesTable: React.FC<MapValuesTableProps> = ({ values, unit, frequency }) => {
  const rows = [...values].sort((a, b) => b.numericValue - a.numericValue);
  return (
    <Accordion
      disableGutters
      variant='outlined'
      sx={{ mt: 2 }}
      TransitionProps={{ unmountOnExit: true }}
    >
      <AccordionSummary expandIcon={<ExpandMore />}>
        <Typography variant='body2'>
          Values as a table ({rows.length === 1 ? '1 country' : `${rows.length} countries`})
        </Typography>
      </AccordionSummary>
      <AccordionDetails sx={{ maxHeight: 400, overflow: 'auto', p: 0 }}>
        <Table size='small' stickyHeader aria-label='Values by country'>
          <TableHead>
            <TableRow>
              <TableCell>Country</TableCell>
              <TableCell align='right'>Value{unit ? ` (${unit})` : ''}</TableCell>
              <TableCell>Date</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {rows.map(row => (
              <TableRow key={row.key}>
                <TableCell>
                  <Link component={RouterLink} to={`/series/${row.seriesId}`}>
                    {row.name}
                  </Link>
                </TableCell>
                <TableCell align='right'>{formatMapValue(row.value)}</TableCell>
                <TableCell>{formatMapDate(row.date, frequency)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </AccordionDetails>
    </Accordion>
  );
};

export default MapValuesTable;
