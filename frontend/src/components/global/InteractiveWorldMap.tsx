/**
 * InteractiveWorldMap Component.
 *
 * A D3.js-powered interactive world map component for global economic analysis.
 * Features include country selection, zoom/pan, economic data visualization,
 * and responsive design.
 */

import React, { useRef, useEffect, useState } from 'react';
import * as d3 from 'd3';
import * as topojson from 'topojson-client';
import { Box, CircularProgress, Alert } from '@mui/material';
import { CountryData, MapViewState } from '../../types/globalAnalysis';
import { useWorldMap } from './hooks/useWorldMap';
import { useCountryData } from './hooks/useCountryData';

interface InteractiveWorldMapProps {
  /** Array of country data to display on the map. */
  data: CountryData[];
  /** Currently selected economic indicator. */
  selectedIndicator: string;
  /** Time range for data visualization. */
  timeRange: { start: Date; end: Date };
  /** Callback when a country is clicked. */
  onCountryClick: (country: CountryData) => void;
  /** Callback when a country is hovered. */
  onCountryHover: (country: CountryData | null) => void;
  /** Current map view state. */
  mapView: MapViewState;
  /** Callback when map view changes. */
  onMapViewChange: (view: Partial<MapViewState>) => void;
  /** Whether animation is enabled. */
  animationEnabled?: boolean;
  /** Whether to show country borders. */
  showBorders?: boolean;
  /** Whether to show country labels. */
  showLabels?: boolean;
  /** Size of country labels. */
  labelSize?: number;
  /** Width of the map container. */
  width: number;
  /** Height of the map container. */
  height: number;
  /** Map projection type. */
  projection?: string;
  /** Color scheme for data visualization. */
  colorScheme?: string;
}

const InteractiveWorldMap: React.FC<InteractiveWorldMapProps> = ({
  data,
  selectedIndicator,
  timeRange: _timeRange,
  onCountryClick,
  onCountryHover,
  mapView: _mapView,
  onMapViewChange: _onMapViewChange,
  animationEnabled: _animationEnabled = true,
  showBorders = true,
  showLabels = false,
  labelSize = 12,
  width,
  height,
  projection = 'naturalEarth',
  colorScheme = 'viridis',
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const [worldData, setWorldData] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Custom hooks for map logic
  const { path, zoomBehavior } = useWorldMap(svgRef, projection);
  const { processedData, colorScale } = useCountryData(data, selectedIndicator, colorScheme);

  // Load world atlas data
  useEffect(() => {
    const loadWorldData = async () => {
      try {
        setLoading(true);
        setError(null);

        // Load world atlas data from a CDN or local file
        const response = await fetch('https://cdn.jsdelivr.net/npm/world-atlas@3/world/110m.json');
        const world = await response.json();
        setWorldData(world);
      } catch (err) {
        // Handle error silently or log to monitoring service
        setError('Failed to load world map data');
      } finally {
        setLoading(false);
      }
    };

    loadWorldData();
  }, []);

  // Initialize D3 map
  useEffect(() => {
    if (!worldData || !svgRef.current) return;

    const svg = d3.select(svgRef.current);
    svg.selectAll('*').remove(); // Clear previous content

    // Set up SVG dimensions
    svg
      .attr('width', width)
      .attr('height', height)
      .attr('viewBox', `0 0 ${width} ${height}`)
      .style('background-color', '#f5f5f5')
      .style('border-radius', '8px')
      .style('box-shadow', '0 2px 8px rgba(0,0,0,0.1)');

    // Create map container
    const mapContainer = svg.append('g').attr('class', 'map-container');

    // Create countries group
    const countriesGroup = mapContainer.append('g').attr('class', 'countries');

    // Create borders group
    const bordersGroup = mapContainer.append('g').attr('class', 'borders');

    // Create labels group
    // const labelsGroup = mapContainer.append('g').attr('class', 'labels');

    // Render world map
    const countries = worldData.objects.countries;
    const countriesPath = topojson.feature(worldData, countries) as any;

    // Draw countries
    countriesGroup
      .selectAll('path.country')
      .data(countriesPath.features)
      .enter()
      .append('path')
      .attr('class', 'country')
      .attr('d', (d: any) => path(d))
      .style('fill', (d: any) => {
        const countryCode = d.properties.ISO_A2;
        const countryData = processedData.find(c => c.isoAlpha2 === countryCode);

        if (countryData && countryData.economicIndicators) {
          const indicator = countryData.economicIndicators.find(
            ind => ind.name === selectedIndicator
          );
          return indicator ? colorScale(indicator.value) : '#d0d0d0';
        }
        return '#d0d0d0';
      })
      .style('stroke', showBorders ? '#ffffff' : '#cccccc')
      .style('stroke-width', showBorders ? '1' : '0.5')
      .style('opacity', '0.9')
      .style('cursor', 'pointer')
      .on('click', (event: MouseEvent, d: any) => {
        const countryCode = d.properties.ISO_A2;
        const countryData = processedData.find(c => c.isoAlpha2 === countryCode);
        if (countryData) {
          onCountryClick(countryData);
        }
      })
      .on('mouseover', (event: MouseEvent, d: any) => {
        const countryCode = d.properties.ISO_A2;
        const countryData = processedData.find(c => c.isoAlpha2 === countryCode);
        if (countryData) {
          onCountryHover(countryData);
          d3.select(event.currentTarget as SVGElement)
            .style('stroke', '#1976d2')
            .style('stroke-width', '3')
            .style('opacity', '1')
            .style('filter', 'drop-shadow(0 2px 4px rgba(0,0,0,0.3))');
        }
      })
      .on('mouseout', (event: MouseEvent) => {
        onCountryHover(null);
        d3.select(event.currentTarget as SVGElement)
          .style('stroke', showBorders ? '#ffffff' : '#cccccc')
          .style('stroke-width', showBorders ? '1' : '0.5')
          .style('opacity', '0.9')
          .style('filter', 'none');
      });

    // Draw country borders
    if (showBorders) {
      bordersGroup
        .append('path')
        .datum(topojson.mesh(worldData, countries, (a, b) => a !== b))
        .attr('class', 'border')
        .attr('d', (d: any) => path(d))
        .style('fill', 'none')
        .style('stroke', '#ffffff')
        .style('stroke-width', '1')
        .style('opacity', '0.8');
    }

    // Add country labels
    if (showLabels) {
      countriesGroup
        .selectAll('text.country-label')
        .data(countriesPath.features)
        .enter()
        .append('text')
        .attr('class', 'country-label')
        .attr('x', (d: any) => {
          const centroid = path.centroid(d);
          return centroid[0];
        })
        .attr('y', (d: any) => {
          const centroid = path.centroid(d);
          return centroid[1];
        })
        .attr('text-anchor', 'middle')
        .attr('font-size', labelSize)
        .attr('font-family', 'Arial, sans-serif')
        .attr('fill', '#2c3e50')
        .attr('font-weight', '500')
        .text((d: any) => d.properties.NAME)
        .style('pointer-events', 'none')
        .style('text-shadow', '1px 1px 2px rgba(255,255,255,0.8)');
    }

    // Set up zoom behavior
    svg.call(zoomBehavior);
  }, [
    worldData,
    processedData,
    selectedIndicator,
    colorScale,
    width,
    height,
    showBorders,
    showLabels,
    labelSize,
    path,
    zoomBehavior,
    onCountryClick,
    onCountryHover,
  ]);

  // Handle loading state
  if (loading) {
    return (
      <Box display='flex' justifyContent='center' alignItems='center' height={height} width={width}>
        <CircularProgress />
      </Box>
    );
  }

  // Handle error state
  if (error) {
    return (
      <Box display='flex' justifyContent='center' alignItems='center' height={height} width={width}>
        <Alert severity='error'>{error}</Alert>
      </Box>
    );
  }

  return (
    <Box
      width={width}
      height={height}
      border='2px solid #e0e0e0'
      borderRadius={2}
      overflow='hidden'
      boxShadow='0 4px 12px rgba(0,0,0,0.15)'
      bgcolor='#fafafa'
    >
      <svg
        ref={svgRef}
        width={width}
        height={height}
        style={{
          display: 'block',
          borderRadius: '6px',
        }}
      />
    </Box>
  );
};

export default InteractiveWorldMap;
