/**
 * InteractiveWorldMap Component.
 *
 * A D3.js choropleth of one indicator's value per country. Values are joined to the outline's
 * features on the ISO 3166-1 numeric code, which world-atlas uses as feature id. Hovering shows a tooltip;
 * clicking a country with a value calls `onCountryClick`. The map can be zoomed and panned.
 *
 * The SVG is built once per outline, projection and size. New values or a new color scale only
 * restyle the existing country paths, and hovering only updates the tooltip, so neither rebuilds
 * the map.
 */

import React, { useRef, useEffect, useLayoutEffect, useState, useCallback, Suspense } from 'react';
import * as d3 from 'd3';
import * as topojson from 'topojson-client';
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  IconButton,
  Tooltip,
  Typography,
} from '@mui/material';
import { Add, Remove, CenterFocusStrong } from '@mui/icons-material';
import { useQuery } from '@tanstack/react-query';
import { useWorldMap, MAX_ZOOM, MIN_ZOOM } from './hooks/useWorldMap';
import type { MapCountryValue } from './hooks/useWorldMapData';
import { worldAtlasQuery } from './worldAtlas';
import CountryTooltip from './CountryTooltip';
import { NO_DATA_FILL } from './mapFormat';

/** Zoom factor of one press of the zoom buttons. */
const ZOOM_STEP = 1.5;

export interface InteractiveWorldMapProps {
  /** Values to show, keyed by ISO numeric code. */
  valuesByIsoNumeric: ReadonlyMap<number, MapCountryValue>;
  /** Maps a value to its fill. */
  colorScale: (value: number) => string;
  /** Unit of the values, for the tooltip. */
  unit?: string | null;
  /** Series frequency, e.g. `Annual`, for formatting dates in the tooltip. */
  frequency?: string | null;
  /** Called when a country with a value is clicked. */
  onCountryClick?: (country: MapCountryValue) => void;
  /** Whether to show country borders. */
  showBorders?: boolean;
  /** Whether to show country labels. */
  showLabels?: boolean;
  /** Size of country labels. */
  labelSize?: number;
  /** Drawing width; the map scales down to fit a narrower container. */
  width: number;
  /** Drawing height. */
  height: number;
  /** Map projection type, a key of `PROJECTIONS`. */
  projection?: string;
}

/** The country under the pointer. */
interface HoverState {
  isoNumeric: number | null;
  name: string;
  /** Pointer position within the map's box, in pixels. */
  x: number;
  y: number;
  /** The map box's size, to keep the tooltip inside it. */
  boxWidth: number;
  boxHeight: number;
}

// Load the bundled world atlas, suspending until it is in
const useWorldAtlasData = () => useQuery({ ...worldAtlasQuery, suspense: true });

/**
 * The ISO numeric code of a world-atlas feature.
 * @param feature - A country feature; its id is the code as a zero-padded string, e.g. `"040"`.
 * @returns The code, or null for the few features without one.
 */
const featureIsoNumeric = (feature: any): number | null => {
  if (feature.id === undefined || feature.id === null || feature.id === '') return null;
  const code = Number(feature.id);
  return Number.isInteger(code) ? code : null;
};

// Main map content component - assumes data is loaded
const WorldMapContent: React.FC<InteractiveWorldMapProps> = ({
  valuesByIsoNumeric,
  colorScale,
  unit = null,
  frequency = null,
  onCountryClick,
  showBorders = true,
  showLabels = false,
  labelSize = 12,
  width,
  height,
  projection = 'naturalEarth',
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<HoverState | null>(null);
  // How the last press on a country was made ('mouse', 'touch' or 'pen'), and the country
  // last tapped, so that a tap can show the tooltip before a second one opens the country.
  const lastPointerType = useRef<string | null>(null);
  const tappedIsoNumeric = useRef<number | null>(null);

  // Load world atlas data with Suspense
  const { data: worldData, isError } = useWorldAtlasData();

  const { path, zoomBehavior, zoomLevel, isTransformed, zoomBy, resetZoom } = useWorldMap(
    svgRef,
    projection,
    [width, height]
  );

  // The SVG's event handlers read the latest values and callback from here, so new values or a
  // new callback don't need new handlers, and the map isn't rebuilt for them.
  const latest = useRef({ valuesByIsoNumeric, colorScale, onCountryClick });
  // A layout effect runs before the effects below, so they see this render's props.
  useLayoutEffect(() => {
    latest.current = { valuesByIsoNumeric, colorScale, onCountryClick };
  });

  // Color each country path by its value.
  const applyFills = useCallback(() => {
    if (!svgRef.current) return;
    const { valuesByIsoNumeric: values, colorScale: scale } = latest.current;
    d3.select(svgRef.current)
      .selectAll('path.country')
      .each(function (d: any) {
        const isoNumeric = featureIsoNumeric(d);
        const entry = isoNumeric === null ? undefined : values.get(isoNumeric);
        d3.select(this as SVGElement)
          .attr('data-has-data', entry ? 'true' : 'false')
          .style('fill', entry ? scale(entry.numericValue) : NO_DATA_FILL)
          .style('cursor', entry ? 'pointer' : 'default');
      });
  }, []);

  // Build the map
  useEffect(() => {
    if (!worldData || !svgRef.current) return;

    const svg = d3.select(svgRef.current);
    svg.selectAll('*').remove(); // Clear previous content

    svg
      .attr('viewBox', `0 0 ${width} ${height}`)
      .style('background-color', '#f5f5f5')
      .style('border-radius', '8px');

    // Create map container, keeping any zoom the user already applied
    const mapContainer = svg
      .append('g')
      .attr('class', 'map-container')
      .attr('transform', d3.zoomTransform(svg.node() as SVGSVGElement).toString());

    const countriesGroup = mapContainer.append('g').attr('class', 'countries');
    const bordersGroup = mapContainer.append('g').attr('class', 'borders');

    const countries = worldData.objects.countries;
    const countriesPath = topojson.feature(worldData, countries) as any;

    const borderStroke = showBorders ? '#ffffff' : '#cccccc';
    const borderWidth = showBorders ? '1' : '0.5';

    const pointerIn = (event: MouseEvent, d: any) => {
      const box = boxRef.current?.getBoundingClientRect();
      setHover({
        isoNumeric: featureIsoNumeric(d),
        name: d.properties?.name ?? '',
        x: box ? event.clientX - box.left : 0,
        y: box ? event.clientY - box.top : 0,
        boxWidth: box?.width ?? 0,
        boxHeight: box?.height ?? 0,
      });
      d3.select(event.currentTarget as SVGElement)
        .style('stroke', '#1976d2')
        .style('stroke-width', '2');
    };

    countriesGroup
      .selectAll('path.country')
      .data(countriesPath.features)
      .enter()
      .append('path')
      .attr('class', 'country')
      .attr('data-iso-numeric', (d: any) => featureIsoNumeric(d) ?? '')
      .attr('data-name', (d: any) => d.properties?.name ?? '')
      .attr('d', (d: any) => path(d))
      .style('stroke', borderStroke)
      .style('stroke-width', borderWidth)
      .style('vector-effect', 'non-scaling-stroke')
      .style('opacity', '0.9')
      .on('pointerdown', (event: { pointerType: string }) => {
        lastPointerType.current = event.pointerType;
      })
      .on('click', (_event: MouseEvent, d: any) => {
        const isoNumeric = featureIsoNumeric(d);
        const entry =
          isoNumeric === null ? undefined : latest.current.valuesByIsoNumeric.get(isoNumeric);
        // A tap is also the touch hover: the first tap on a country shows its tooltip and
        // a second tap opens it.
        if (lastPointerType.current === 'touch' && tappedIsoNumeric.current !== isoNumeric) {
          tappedIsoNumeric.current = isoNumeric;
          return;
        }
        if (!entry) return;
        tappedIsoNumeric.current = null;
        latest.current.onCountryClick?.(entry);
      })
      .on('mouseover', pointerIn)
      .on('mousemove', pointerIn)
      .on('mouseout', (event: MouseEvent) => {
        setHover(null);
        d3.select(event.currentTarget as SVGElement)
          .style('stroke', borderStroke)
          .style('stroke-width', borderWidth);
      });

    applyFills();

    if (showBorders) {
      bordersGroup
        .append('path')
        .datum(topojson.mesh(worldData, countries, (a, b) => a !== b))
        .attr('class', 'border')
        .attr('d', (d: any) => path(d))
        .style('fill', 'none')
        .style('stroke', '#ffffff')
        .style('stroke-width', '1')
        .style('vector-effect', 'non-scaling-stroke')
        .style('opacity', '0.8')
        .style('pointer-events', 'none');
    }

    if (showLabels) {
      countriesGroup
        .selectAll('text.country-label')
        .data(countriesPath.features)
        .enter()
        .append('text')
        .attr('class', 'country-label')
        .attr('x', (d: any) => path.centroid(d)[0])
        .attr('y', (d: any) => path.centroid(d)[1])
        .attr('text-anchor', 'middle')
        .attr('font-size', labelSize)
        .attr('font-family', 'Arial, sans-serif')
        .attr('fill', '#2c3e50')
        .attr('font-weight', '500')
        .text((d: any) => d.properties.name)
        .style('pointer-events', 'none')
        .style('text-shadow', '1px 1px 2px rgba(255,255,255,0.8)');
    }

    // Set up zoom behavior; useWorldMap creates it in an effect, so it is null
    // on the first render.
    if (zoomBehavior) {
      svg.call(zoomBehavior);
    }
  }, [
    worldData,
    width,
    height,
    showBorders,
    showLabels,
    labelSize,
    path,
    zoomBehavior,
    applyFills,
  ]);

  // Restyle, without rebuilding, when the values or the color scale change.
  useEffect(() => {
    applyFills();
  }, [valuesByIsoNumeric, colorScale, applyFills]);

  if (isError) {
    return (
      <Box display='flex' justifyContent='center' alignItems='center' width={width} height={height}>
        <Alert
          severity='error'
          action={
            <Button color='inherit' size='small' onClick={() => window.location.reload()}>
              Reload
            </Button>
          }
        >
          Failed to load world map data
        </Alert>
      </Box>
    );
  }

  const hoveredEntry =
    hover && hover.isoNumeric !== null ? valuesByIsoNumeric.get(hover.isoNumeric) : undefined;

  return (
    <Box
      ref={boxRef}
      position='relative'
      width='100%'
      maxWidth={width}
      border='2px solid #e0e0e0'
      borderRadius={2}
      overflow='hidden'
      bgcolor='#fafafa'
      data-testid='world-map'
    >
      <svg
        ref={svgRef}
        role='img'
        aria-label='World map'
        style={{
          display: 'block',
          width: '100%',
          height: 'auto',
          aspectRatio: `${width} / ${height}`,
        }}
      />
      {/* Overlaid on the map's top-right corner, except on a phone, where an overlay that size
          would cover Europe and East Asia; there the controls sit in a bar under the map. */}
      <Box
        data-testid='map-zoom-controls'
        sx={{
          position: { xs: 'static', sm: 'absolute' },
          top: 8,
          right: 8,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'flex-end',
          gap: 0.5,
          bgcolor: 'rgba(255,255,255,0.85)',
          borderRadius: { xs: 0, sm: 1 },
          borderTop: { xs: '1px solid #e0e0e0', sm: 'none' },
          px: 0.5,
        }}
      >
        <Typography variant='caption' aria-live='polite' sx={{ minWidth: 40, textAlign: 'right' }}>
          {Math.round(zoomLevel * 100)}%
        </Typography>
        <Tooltip title='Zoom in'>
          <span>
            <IconButton
              size='small'
              aria-label='Zoom in'
              onClick={() => zoomBy(ZOOM_STEP)}
              disabled={zoomLevel >= MAX_ZOOM}
            >
              <Add fontSize='small' />
            </IconButton>
          </span>
        </Tooltip>
        <Tooltip title='Zoom out'>
          <span>
            <IconButton
              size='small'
              aria-label='Zoom out'
              onClick={() => zoomBy(1 / ZOOM_STEP)}
              disabled={zoomLevel <= MIN_ZOOM}
            >
              <Remove fontSize='small' />
            </IconButton>
          </span>
        </Tooltip>
        <Tooltip title='Reset zoom'>
          <span>
            <IconButton
              size='small'
              aria-label='Reset zoom'
              onClick={resetZoom}
              disabled={!isTransformed}
            >
              <CenterFocusStrong fontSize='small' />
            </IconButton>
          </span>
        </Tooltip>
      </Box>
      {hover && (
        <CountryTooltip
          name={hoveredEntry?.name ?? hover.name}
          entry={hoveredEntry}
          unit={unit}
          frequency={frequency}
          x={hover.x}
          y={hover.y}
          boxWidth={hover.boxWidth}
          boxHeight={hover.boxHeight}
        />
      )}
    </Box>
  );
};

// Wrapper component with Suspense boundary
const InteractiveWorldMap: React.FC<InteractiveWorldMapProps> = props => {
  return (
    <Suspense
      fallback={
        <Box
          display='flex'
          justifyContent='center'
          alignItems='center'
          height={props.height}
          width='100%'
          maxWidth={props.width}
        >
          <CircularProgress />
        </Box>
      }
    >
      <WorldMapContent {...props} />
    </Suspense>
  );
};

export default InteractiveWorldMap;
