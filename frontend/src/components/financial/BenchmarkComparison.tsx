import React from 'react';

interface BenchmarkData {
  percentile: number;
  industryMedian: number;
  industryP25: number;
  industryP75: number;
  industryP90: number;
  industryP10: number;
}

interface BenchmarkComparisonProps {
  ratioName: string;
  companyValue: number;
  benchmarkData?: BenchmarkData;
}

export const BenchmarkComparison = React.memo<BenchmarkComparisonProps>(
  ({ ratioName, companyValue, benchmarkData }) => {
    if (!benchmarkData) {
      return (
        <div className='benchmark-comparison p-4 bg-gray-50 rounded-lg'>
          <h4 className='text-sm font-medium text-gray-700 mb-2'>
            Industry Benchmark: {ratioName}
          </h4>
          <p className='text-sm text-gray-600'>No benchmark data available for this ratio.</p>
        </div>
      );
    }

    const getPercentileColor = (percentile: number) => {
      if (percentile >= 75) return 'text-green-600';
      if (percentile >= 50) return 'text-blue-600';
      if (percentile >= 25) return 'text-yellow-600';
      return 'text-red-600';
    };

    const getPercentileLabel = (percentile: number) => {
      if (percentile >= 90) return 'Top 10%';
      if (percentile >= 75) return 'Top 25%';
      if (percentile >= 50) return 'Above Median';
      if (percentile >= 25) return 'Below Median';
      return 'Bottom 25%';
    };

    return (
      <div
        className='benchmark-comparison p-4 bg-gray-50 rounded-lg'
        role='region'
        aria-label={`Industry benchmark data for ${ratioName}`}
      >
        <h4 className='text-sm font-medium text-gray-700 mb-3' id={`benchmark-title-${ratioName}`}>
          Industry Benchmark: {ratioName}
        </h4>

        <div className='space-y-3'>
          <div className='flex justify-between items-center'>
            <span className='text-sm text-gray-600' id={`company-value-label-${ratioName}`}>
              Company Value:
            </span>
            <span className='font-medium' aria-labelledby={`company-value-label-${ratioName}`}>
              {companyValue.toFixed(2)}
            </span>
          </div>

          <div className='flex justify-between items-center'>
            <span className='text-sm text-gray-600' id={`percentile-label-${ratioName}`}>
              Industry Percentile:
            </span>
            <span
              className={`font-medium ${getPercentileColor(benchmarkData.percentile)}`}
              aria-labelledby={`percentile-label-${ratioName}`}
              aria-label={`Company ranks at ${benchmarkData.percentile.toFixed(1)} percentile, ${getPercentileLabel(benchmarkData.percentile)}`}
            >
              {benchmarkData.percentile.toFixed(1)}% ({getPercentileLabel(benchmarkData.percentile)}
              )
            </span>
          </div>

          <div className='border-t pt-3'>
            <h5
              className='text-xs font-medium text-gray-600 mb-2'
              id={`distribution-title-${ratioName}`}
            >
              Industry Distribution
            </h5>
            <div
              className='grid grid-cols-2 gap-2 text-xs'
              role='table'
              aria-labelledby={`distribution-title-${ratioName}`}
            >
              <div className='flex justify-between' role='row'>
                <span className='text-gray-500' role='rowheader'>
                  P10:
                </span>
                <span
                  role='cell'
                  aria-label={`10th percentile value: ${benchmarkData.industryP10.toFixed(2)}`}
                >
                  {benchmarkData.industryP10.toFixed(2)}
                </span>
              </div>
              <div className='flex justify-between' role='row'>
                <span className='text-gray-500' role='rowheader'>
                  P25:
                </span>
                <span
                  role='cell'
                  aria-label={`25th percentile value: ${benchmarkData.industryP25.toFixed(2)}`}
                >
                  {benchmarkData.industryP25.toFixed(2)}
                </span>
              </div>
              <div className='flex justify-between' role='row'>
                <span className='text-gray-500' role='rowheader'>
                  Median:
                </span>
                <span
                  className='font-medium'
                  role='cell'
                  aria-label={`Median value: ${benchmarkData.industryMedian.toFixed(2)}`}
                >
                  {benchmarkData.industryMedian.toFixed(2)}
                </span>
              </div>
              <div className='flex justify-between' role='row'>
                <span className='text-gray-500' role='rowheader'>
                  P75:
                </span>
                <span
                  role='cell'
                  aria-label={`75th percentile value: ${benchmarkData.industryP75.toFixed(2)}`}
                >
                  {benchmarkData.industryP75.toFixed(2)}
                </span>
              </div>
              <div className='flex justify-between' role='row'>
                <span className='text-gray-500' role='rowheader'>
                  P90:
                </span>
                <span
                  role='cell'
                  aria-label={`90th percentile value: ${benchmarkData.industryP90.toFixed(2)}`}
                >
                  {benchmarkData.industryP90.toFixed(2)}
                </span>
              </div>
            </div>
          </div>

          {/* Performance Rating - Dynamic based on percentile */}
          <div className='mt-3 text-center'>
            <span
              className={`text-sm font-medium ${
                benchmarkData.percentile >= 75
                  ? 'text-green-600'
                  : benchmarkData.percentile >= 50
                    ? 'text-blue-600'
                    : 'text-red-600'
              }`}
            >
              {benchmarkData.percentile >= 75
                ? 'Above Average'
                : benchmarkData.percentile >= 50
                  ? 'Average'
                  : 'Below Average'}
            </span>
          </div>
        </div>
      </div>
    );
  }
);

BenchmarkComparison.displayName = 'BenchmarkComparison';
