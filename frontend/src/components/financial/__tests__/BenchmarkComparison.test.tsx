import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { BenchmarkComparison } from '../BenchmarkComparison';

const mockBenchmarkData = {
  ratioName: 'returnOnEquity',
  ratioDisplayName: 'Return on Equity',
  companyValue: 0.147,
  benchmarkData: {
    percentile: 75,
    industryAverage: 0.12,
    median: 0.115,
    topQuartile: 0.15,
    bottomQuartile: 0.08,
    sampleSize: 150,
    lastUpdated: '2024-01-15T00:00:00Z',
    industryP10: 0.08,
    industryP25: 0.10,
    industryMedian: 0.115,
    industryP75: 0.15,
    industryP90: 0.18,
  },
  interpretation: 'Strong profitability, above industry average',
  category: 'profitability',
};

describe('BenchmarkComparison', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('renders the benchmark comparison component', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText('Return on Equity')).toBeInTheDocument();
    expect(screen.getByText('14.7%')).toBeInTheDocument(); // Formatted company value
    expect(screen.getByText('75th Percentile')).toBeInTheDocument();
  });

  it('displays company value with proper formatting', () => {
    render(
      <BenchmarkComparison
        ratioName="currentRatio"
        ratioDisplayName="Current Ratio"
        companyValue={1.04}
        benchmarkData={{
          percentile: 45,
          industryAverage: 1.2,
          median: 1.15,
          topQuartile: 1.5,
          bottomQuartile: 0.9,
          sampleSize: 100,
          lastUpdated: '2024-01-15T00:00:00Z',
          industryP10: 0.9,
          industryP25: 1.0,
          industryMedian: 1.15,
          industryP75: 1.5,
          industryP90: 1.8,
        }}
        interpretation="Adequate liquidity"
        category="liquidity"
      />
    );
    
    expect(screen.getByText('1.04')).toBeInTheDocument();
  });

  it('shows percentile ranking', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText('75th Percentile')).toBeInTheDocument();
  });

  it('displays industry comparison metrics', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText('Industry Average: 12.0%')).toBeInTheDocument();
    expect(screen.getByText('Median: 11.5%')).toBeInTheDocument();
  });

  it('shows quartile ranges', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText('Top Quartile: 15.0%')).toBeInTheDocument();
    expect(screen.getByText('Bottom Quartile: 8.0%')).toBeInTheDocument();
  });

  it('displays interpretation text', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText('Strong profitability, above industry average')).toBeInTheDocument();
  });

  it('shows sample size information', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText('Based on 150 companies')).toBeInTheDocument();
  });

  it('displays last updated timestamp', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText(/Last updated: Jan 15, 2024/i)).toBeInTheDocument();
  });

  it('shows performance indicator colors', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    // Should show green indicator for above-average performance
    const indicator = screen.getByTestId('performance-indicator');
    expect(indicator).toHaveClass('text-green-600');
  });

  it('handles different performance levels', () => {
    render(
      <BenchmarkComparison
        ratioName="debtToEquity"
        ratioDisplayName="Debt to Equity"
        companyValue={2.5}
        benchmarkData={{
          percentile: 25,
          industryAverage: 1.8,
          median: 1.6,
          topQuartile: 2.2,
          bottomQuartile: 1.2,
          sampleSize: 120,
          lastUpdated: '2024-01-15T00:00:00Z',
          industryP10: 1.2,
          industryP25: 1.4,
          industryMedian: 1.6,
          industryP75: 2.2,
          industryP90: 2.8,
        }}
        interpretation="High leverage, above industry average"
        category="leverage"
      />
    );
    
    expect(screen.getByText('25th Percentile')).toBeInTheDocument();
    expect(screen.getByText('High leverage, above industry average')).toBeInTheDocument();
  });

  it('shows detailed benchmark breakdown', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        showDetails={true}
      />
    );
    
    expect(screen.getByText('Detailed Benchmark Analysis')).toBeInTheDocument();
  });

  it('handles missing benchmark data gracefully', () => {
    render(
      <BenchmarkComparison
        ratioName="customRatio"
        ratioDisplayName="Custom Ratio"
        companyValue={0.5}
        benchmarkData={null}
        interpretation="No benchmark data available"
        category="custom"
      />
    );
    
    expect(screen.getByText('No benchmark data available for this ratio.')).toBeInTheDocument();
    expect(screen.getByText('50.0%')).toBeInTheDocument();
  });

  it('displays benchmark chart when enabled', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        showChart={true}
      />
    );
    
    expect(screen.getByTestId('benchmark-chart')).toBeInTheDocument();
  });

  it('handles chart interaction events', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        showChart={true}
      />
    );
    
    const chart = screen.getByTestId('benchmark-chart');
    fireEvent.mouseOver(chart);
    
    // Should handle hover events
    expect(chart).toBeInTheDocument();
  });

  it('shows export functionality', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        showExport={true}
      />
    );
    
    expect(screen.getByText('Export Benchmark Data')).toBeInTheDocument();
  });

  it('handles export button click', () => {
    const mockOnExport = jest.fn();
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        showExport={true}
        onExport={mockOnExport}
      />
    );
    
    const exportButton = screen.getByText('Export Benchmark Data');
    fireEvent.click(exportButton);
    
    expect(mockOnExport).toHaveBeenCalledWith({
      ratioName: mockBenchmarkData.ratioName,
      companyValue: mockBenchmarkData.companyValue,
      benchmarkData: mockBenchmarkData.benchmarkData,
    });
  });

  it('displays educational content for beginners', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        userType="beginner"
        showEducationalContent={true}
      />
    );
    
    expect(screen.getByText('What does this mean?')).toBeInTheDocument();
    expect(screen.getByText('Percentile ranking shows how your company compares to others in the industry.')).toBeInTheDocument();
  });

  it('handles different user types', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        userType="expert"
      />
    );
    
    // Should show more technical details for experts
    expect(screen.getByText('Statistical Significance')).toBeInTheDocument();
  });

  it('shows benchmark methodology information', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        showMethodology={true}
      />
    );
    
    expect(screen.getByText('Benchmark Methodology')).toBeInTheDocument();
    expect(screen.getByText('Data Sources: SEC filings, industry reports')).toBeInTheDocument();
  });

  it('handles responsive design', () => {
    // Mock mobile viewport
    Object.defineProperty(window, 'innerWidth', {
      writable: true,
      configurable: true,
      value: 375,
    });

    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    // Should adapt to mobile view
    expect(screen.getByText('Return on Equity')).toBeInTheDocument();
  });

  it('displays comparison with previous periods', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        previousPeriodData={{
          companyValue: 0.142,
          percentile: 72,
          period: 'Q3 2023',
        }}
      />
    );
    
    expect(screen.getByText('Previous Period (Q3 2023)')).toBeInTheDocument();
    expect(screen.getByText('14.2% (72nd percentile)')).toBeInTheDocument();
  });

  it('shows trend indicators', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={mockBenchmarkData.benchmarkData}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
        trend="improving"
      />
    );
    
    expect(screen.getByText('↗ Improving')).toBeInTheDocument();
  });

  it('handles different trend directions', () => {
    render(
      <BenchmarkComparison
        ratioName="debtToEquity"
        ratioDisplayName="Debt to Equity"
        companyValue={2.5}
        benchmarkData={{
          percentile: 25,
          industryAverage: 1.8,
          median: 1.6,
          topQuartile: 2.2,
          bottomQuartile: 1.2,
          sampleSize: 120,
          lastUpdated: '2024-01-15T00:00:00Z',
          industryP10: 1.2,
          industryP25: 1.4,
          industryMedian: 1.6,
          industryP75: 2.2,
          industryP90: 2.8,
        }}
        interpretation="High leverage"
        category="leverage"
        trend="declining"
      />
    );
    
    expect(screen.getByText('↘ Declining')).toBeInTheDocument();
  });

  it('displays benchmark confidence level', () => {
    render(
      <BenchmarkComparison
        ratioName={mockBenchmarkData.ratioName}
        ratioDisplayName={mockBenchmarkData.ratioDisplayName}
        companyValue={mockBenchmarkData.companyValue}
        benchmarkData={{
          ...mockBenchmarkData.benchmarkData,
          confidenceLevel: 0.95,
        }}
        interpretation={mockBenchmarkData.interpretation}
        category={mockBenchmarkData.category}
      />
    );
    
    expect(screen.getByText('Confidence Level: 95%')).toBeInTheDocument();
  });
});
