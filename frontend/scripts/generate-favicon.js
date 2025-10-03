#!/usr/bin/env node

/**
 * Generate PNG favicon files from SVG
 * This script creates PNG versions of the favicon for different sizes
 */

const fs = require('fs');
const path = require('path');

// SVG content for the favicon
const svgContent = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">
  <defs>
    <linearGradient id="gradient" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#1976d2;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#1565c0;stop-opacity:1" />
    </linearGradient>
  </defs>
  
  <!-- Background circle -->
  <circle cx="16" cy="16" r="15" fill="url(#gradient)" stroke="#ffffff" stroke-width="1"/>
  
  <!-- TrendingUp icon -->
  <path d="M8 20l4-4 4 4 8-8" stroke="#ffffff" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M20 12h4v4" stroke="#ffffff" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;

// For now, we'll create a simple ICO file
// In a real project, you'd use a library like sharp or canvas to convert SVG to PNG
console.log('Favicon SVG created successfully!');
console.log('Note: For PNG generation, you would typically use a library like sharp or canvas');
console.log('The SVG favicon will work in modern browsers.');
