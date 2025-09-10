#!/bin/bash

echo "🎤 Creating professional narration for Global Economic Network Analysis Demo..."

# Create narration directory
mkdir -p demo-videos/narration

# Segment 1: Introduction (0-8 seconds)
say -v Daniel -o "demo-videos/narration/01_intro.aiff" "Welcome to EconGraph's revolutionary Global Economic Network Analysis System. This Bloomberg Terminal-level platform provides institutional-grade economic visualization and analysis."

# Segment 2: Navigation (8-12 seconds)
say -v Daniel -o "demo-videos/narration/02_navigation.aiff" "Let's explore the Global Analysis features that rival premium financial terminals costing over twenty thousand dollars per year."

# Segment 3: Network Map Introduction (12-18 seconds)
say -v Daniel -o "demo-videos/narration/03_network_intro.aiff" "The Interactive Global Economic Network Map visualizes correlations between major economies. Countries are sized by GDP, with connections showing economic relationships."

# Segment 4: Controls Demonstration (18-25 seconds)
say -v Daniel -o "demo-videos/narration/04_controls.aiff" "Professional controls allow you to select different economic indicators, adjust correlation thresholds, and explore network connections in real-time."

# Segment 5: Country Interaction (25-30 seconds)
say -v Daniel -o "demo-videos/narration/05_interaction.aiff" "Click any country to view detailed economic data including GDP, population, and geographic information."

# Segment 6: Multi-Country Dashboard (30-38 seconds)
say -v Daniel -o "demo-videos/narration/06_dashboard.aiff" "The Multi-Country Dashboard provides Bloomberg Terminal-style comparative analysis. Select multiple countries to compare economic indicators side-by-side."

# Segment 7: Dashboard Features (38-45 seconds)
say -v Daniel -o "demo-videos/narration/07_dashboard_features.aiff" "Switch between overview cards, comparison charts, and detailed data tables. All visualizations update dynamically with professional-grade responsiveness."

# Segment 8: Global Events Explorer (45-52 seconds)
say -v Daniel -o "demo-videos/narration/08_events.aiff" "The Global Events Explorer tracks major economic crises including the 2008 Financial Crisis, COVID-19 pandemic, and Brexit with detailed country-specific impact analysis."

# Segment 9: Event Filtering (52-58 seconds)
say -v Daniel -o "demo-videos/narration/09_filtering.aiff" "Advanced filtering allows you to focus on specific event types, adjust impact thresholds, and explore recovery patterns across different countries."

# Segment 10: Event Details (58-65 seconds)
say -v Daniel -o "demo-videos/narration/10_details.aiff" "Expand events to reveal comprehensive impact assessments, recovery status tracking, and detailed descriptions of economic consequences."

# Segment 11: Statistics Overview (65-70 seconds)
say -v Daniel -o "demo-videos/narration/11_statistics.aiff" "Global Impact Statistics provide institutional-level analytics including event counts, country impacts, average severity, and recovery metrics."

# Segment 12: Conclusion (70-78 seconds)
say -v Daniel -o "demo-videos/narration/12_conclusion.aiff" "This revolutionary platform represents the first open-source global economic network analysis system with Bloomberg Terminal-quality capabilities. Professional institutions can now access premium economic analysis tools at no cost."

echo "✅ All narration segments created successfully!"
echo "📁 Audio files saved to demo-videos/narration/"

# List the created files
ls -la demo-videos/narration/
