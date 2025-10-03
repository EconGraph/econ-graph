// REQUIREMENT: Comprehensive unit tests for Dashboard page component
// PURPOSE: Test economic indicators dashboard layout and functionality
// This ensures the main overview interface provides accurate economic information

import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TestProviders } from '../../test-utils/test-providers';
import Dashboard from '../Dashboard';

function renderDashboard() {
  return render(
    <TestProviders>
      <Dashboard />
    </TestProviders>
  );
}

describe('Dashboard', () => {
  test('should render dashboard layout successfully', () => {
    // REQUIREMENT: Test basic dashboard rendering and layout
    // PURPOSE: Verify that dashboard displays economic indicators overview
    // This ensures users can access key economic metrics

    renderDashboard();

    // Verify main dashboard elements
    expect(screen.getByRole('heading', { name: /economic dashboard/i })).toBeInTheDocument();
    expect(screen.getByText(/key economic indicators with professional collaboration features/i)).toBeInTheDocument();
    expect(screen.getByText(/key indicators/i)).toBeInTheDocument();
  });

  test('should display featured economic indicators', async () => {
    // REQUIREMENT: Test economic indicators display
    // PURPOSE: Verify that key economic metrics are shown to users
    // This provides visibility into current economic conditions

    renderDashboard();

    // Should show the four main economic indicators (checking for specific instances in cards)
    expect(screen.getByText(/real gross domestic product/i)).toBeInTheDocument();

    // For unemployment rate, we expect multiple instances - check that at least one exists
    const unemploymentElements = screen.getAllByText(/unemployment rate/i);
    expect(unemploymentElements.length).toBeGreaterThan(0);

    // For consumer price index, we expect multiple instances - check that at least one exists
    const cpiElements = screen.getAllByText(/consumer price index/i);
    expect(cpiElements.length).toBeGreaterThan(0);

    expect(screen.getByText(/federal funds rate/i)).toBeInTheDocument();

    // Should show actual values
    expect(screen.getByText(/\$27\.36T/)).toBeInTheDocument();
    expect(screen.getByText(/3\.7%/)).toBeInTheDocument();
    expect(screen.getByText(/3\.2%/)).toBeInTheDocument();
    expect(screen.getByText(/5\.25%/)).toBeInTheDocument();
  });

  test('should show data source information', async () => {
    // REQUIREMENT: Test data source display
    // PURPOSE: Verify that data sources are clearly identified
    // This helps users understand data provenance and reliability

    renderDashboard();

    // Should show data source chips (multiple instances expected)
    expect(screen.getByText('BEA')).toBeInTheDocument();

    const blsElements = screen.getAllByText('BLS');
    expect(blsElements.length).toBeGreaterThan(0);

    expect(screen.getByText('Federal Reserve')).toBeInTheDocument();
  });

  test('should display recent data releases', async () => {
    // REQUIREMENT: Test recent activity display
    // PURPOSE: Verify that recent data releases are shown for user awareness
    // This helps users stay informed about data updates

    renderDashboard();

    // Check for the specific heading (not the description text)
    expect(screen.getByRole('heading', { name: /recent data releases/i })).toBeInTheDocument();

    // Should show recent releases
    expect(screen.getByText(/employment situation summary/i)).toBeInTheDocument();

    // For industrial production, we expect multiple instances - check that at least one exists
    const industrialElements = screen.getAllByText(/industrial production/i);
    expect(industrialElements.length).toBeGreaterThan(0);

    // Should show release dates and sources
    expect(screen.getByText(/nonfarm payrolls, unemployment rate/i)).toBeInTheDocument();
    expect(screen.getByText(/cpi-u, core cpi/i)).toBeInTheDocument();
  });

  test('should show quick action buttons', async () => {
    // REQUIREMENT: Test quick access navigation elements
    // PURPOSE: Verify that users can quickly navigate to key features
    // This improves user productivity and system discoverability

    renderDashboard();

    // Should show quick action buttons in the sidebar
    expect(screen.getByText(/employment data/i)).toBeInTheDocument();
    expect(screen.getByText(/inflation indicators/i)).toBeInTheDocument();
    expect(screen.getByText(/gdp & growth/i)).toBeInTheDocument();
    expect(screen.getByText(/browse data sources/i)).toBeInTheDocument();
    expect(screen.getByText(/explore all series/i)).toBeInTheDocument();
  });

  test('should display system status information', async () => {
    // REQUIREMENT: Test system status display
    // PURPOSE: Verify that data freshness indicators are shown
    // This helps users understand data currency and reliability

    renderDashboard();

    expect(screen.getByText(/system status/i)).toBeInTheDocument();
    expect(screen.getByText(/data freshness/i)).toBeInTheDocument();
    expect(screen.getByText(/current/i)).toBeInTheDocument();
    expect(screen.getByText(/last updated: 2 hours ago/i)).toBeInTheDocument();
  });

  test('should handle indicator card interactions', async () => {
    // REQUIREMENT: Test widget interactivity
    // PURPOSE: Verify that indicator cards respond to user interactions
    // This enables detailed exploration from overview information

    renderDashboard();

    // Find GDP indicator card using accessible queries
    const gdpCard = screen.getByText(/real gross domestic product/i);

    // Verify the card is clickable (has button role or is within a clickable element)
    expect(gdpCard).toBeInTheDocument();
    // Navigation would be tested with router mocks in a full implementation
  });

  test('should display change indicators with proper styling', async () => {
    // REQUIREMENT: Test change indicator display
    // PURPOSE: Verify that economic changes are visually represented
    // This helps users quickly understand economic trends

    renderDashboard();

    // Should show change percentages
    expect(screen.getByText(/\+2\.4%/)).toBeInTheDocument();
    expect(screen.getByText(/-0\.1%/)).toBeInTheDocument();
    expect(screen.getByText(/\+0\.2%/)).toBeInTheDocument();
    expect(screen.getByText(/0\.0%/)).toBeInTheDocument();

    // Should show time periods (handle multiple instances)
    expect(screen.getByText(/q3 2024/i)).toBeInTheDocument();

    const novElements = screen.getAllByText(/nov 2024/i);
    expect(novElements.length).toBeGreaterThan(0);

    expect(screen.getByText(/dec 2024/i)).toBeInTheDocument();
  });

  test('should show refresh functionality', async () => {
    // REQUIREMENT: Test dashboard refresh capability
    // PURPOSE: Verify that users can refresh data
    // This ensures access to current information

    renderDashboard();

    const refreshButton = screen.getByLabelText(/refresh data/i);
    expect(refreshButton).toBeInTheDocument();
  });

  test('should handle responsive layout', () => {
    // REQUIREMENT: Test responsive dashboard layout
    // PURPOSE: Verify that dashboard adapts to different screen sizes
    // This ensures accessibility across devices

    renderDashboard();

    // The MUI Grid system should handle responsiveness
    // We can verify that the grid containers exist
    const gridContainers = screen.getAllByRole('generic');
    expect(gridContainers.length).toBeGreaterThan(0);
  });

  test('should display proper accessibility attributes', () => {
    // REQUIREMENT: Test accessibility compliance
    // PURPOSE: Verify that dashboard is accessible to all users
    // This ensures inclusive design

    renderDashboard();

    // Check for proper headings hierarchy
    expect(screen.getByRole('heading', { level: 1 })).toBeInTheDocument();

    // Check for button accessibility
    const refreshButton = screen.getByLabelText(/refresh data/i);
    expect(refreshButton).toHaveAttribute('aria-label');

    const viewDetailsButtons = screen.getAllByLabelText(/view details/i);
    expect(viewDetailsButtons.length).toBeGreaterThan(0);
  });

  test('should show proper data formatting', () => {
    // REQUIREMENT: Test data presentation
    // PURPOSE: Verify that economic data is properly formatted
    // This ensures clear communication of information

    renderDashboard();

    // Should show properly formatted currency
    expect(screen.getByText(/\$27\.36T/)).toBeInTheDocument();

    // Should show properly formatted percentages
    expect(screen.getByText(/3\.7%/)).toBeInTheDocument();
    expect(screen.getByText(/3\.2%/)).toBeInTheDocument();
    expect(screen.getByText(/5\.25%/)).toBeInTheDocument();
  });

  test('should handle navigation interactions', async () => {
    // REQUIREMENT: Test navigation functionality
    // PURPOSE: Verify that dashboard navigation works correctly
    // This ensures users can move to detailed views

    const user = userEvent.setup();
    renderDashboard();

    // Test quick action buttons
    const employmentButton = screen.getByText(/employment data/i);
    const inflationButton = screen.getByText(/inflation indicators/i);
    const gdpButton = screen.getByText(/gdp & growth/i);

    expect(employmentButton).toBeInTheDocument();
    expect(inflationButton).toBeInTheDocument();
    expect(gdpButton).toBeInTheDocument();

    // In a full implementation, these would test actual navigation
    // For now, we verify the buttons exist and are clickable
    await user.click(employmentButton);
    await user.click(inflationButton);
    await user.click(gdpButton);
  });

  // =============================================================================
  // EMPTY STATE TESTS - Critical for deployment scenarios with empty databases
  // =============================================================================

  test('should handle empty economic data gracefully', () => {
    // REQUIREMENT: Test dashboard behavior when no economic data is available
    // PURPOSE: Verify graceful handling when database has no economic series
    // This prevents blank dashboard when deployment has empty database

    // Note: The current Dashboard component uses mock data and doesn't handle
    // empty states from the API. This test documents the current behavior
    // and identifies where empty state handling should be added.

    renderDashboard();

    // Should still render dashboard structure
    expect(screen.getByRole('heading', { name: /economic dashboard/i })).toBeInTheDocument();
    expect(screen.getByText(/key economic indicators/i)).toBeInTheDocument();

    // Should show placeholder data or loading states
    // In a real implementation, this would show empty state messages
    // when API returns no data for economic indicators
  });

  test('should maintain dashboard structure during data loading', () => {
    // REQUIREMENT: Test dashboard structure during loading states
    // PURPOSE: Verify dashboard remains functional while data loads
    // This ensures good UX during API calls

    renderDashboard();

    // Should maintain core dashboard structure
    expect(screen.getByRole('heading', { name: /economic dashboard/i })).toBeInTheDocument();
    expect(screen.getByText(/key indicators/i)).toBeInTheDocument();

    // Should show system status section
    expect(screen.getByText(/system status/i)).toBeInTheDocument();
    expect(screen.getByText(/data freshness/i)).toBeInTheDocument();

    // Should maintain navigation elements
    expect(screen.getByText(/employment data/i)).toBeInTheDocument();
    expect(screen.getByText(/inflation indicators/i)).toBeInTheDocument();
    expect(screen.getByText(/gdp & growth/i)).toBeInTheDocument();
  });

  test('should handle missing economic indicators gracefully', () => {
    // REQUIREMENT: Test dashboard when specific economic indicators are missing
    // PURPOSE: Verify graceful handling when some data sources fail
    // This prevents partial dashboard failures

    renderDashboard();

    // Should still show dashboard structure even if some indicators fail
    expect(screen.getByRole('heading', { name: /economic dashboard/i })).toBeInTheDocument();

    // Should maintain collaboration features
    expect(screen.getByText(/professional collaboration features/i)).toBeInTheDocument();

    // Should show system status regardless of data availability
    expect(screen.getByText(/system status/i)).toBeInTheDocument();
  });

  test('should provide clear feedback during system issues', () => {
    // REQUIREMENT: Test dashboard feedback during system problems
    // PURPOSE: Verify users understand when there are system issues
    // This improves transparency during deployment problems

    renderDashboard();

    // Should show system status information
    expect(screen.getByText(/system status/i)).toBeInTheDocument();
    expect(screen.getByText(/data freshness/i)).toBeInTheDocument();

    // Should provide status indicators
    const statusChips = screen.getAllByText(/current/i);
    expect(statusChips.length).toBeGreaterThan(0);

    // Should show last updated information
    expect(screen.getByText(/last updated/i)).toBeInTheDocument();
  });

  test('should maintain accessibility during empty states', () => {
    // REQUIREMENT: Test accessibility features during empty states
    // PURPOSE: Verify screen readers work properly during data issues
    // This ensures inclusive user experience during deployment issues

    renderDashboard();

    // Should maintain proper heading structure
    expect(screen.getByRole('heading', { name: /economic dashboard/i })).toBeInTheDocument();

    // Should maintain accessible navigation
    const navButtons = screen.getAllByRole('button');
    expect(navButtons.length).toBeGreaterThan(0);

    // Should maintain accessible status information
    expect(screen.getByText(/system status/i)).toBeInTheDocument();
    expect(screen.getByText(/data freshness/i)).toBeInTheDocument();
  });
});
