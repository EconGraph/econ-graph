import React, { useState, useMemo } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  TrendingUp,
  TrendingDown,
  BarChart3,
  LineChart,
  Calendar,
  Filter,
  Download,
} from 'lucide-react';
import { FinancialStatement, FinancialRatio } from '@/types/financial';

interface TrendAnalysisChartProps {
  ratios: FinancialRatio[];
  statements: FinancialStatement[];
  timeRange: '1Y' | '3Y' | '5Y' | '10Y';
  onTimeRangeChange: (range: '1Y' | '3Y' | '5Y' | '10Y') => void;
  selectedRatios?: string[];
  onRatioSelectionChange?: (ratios: string[]) => void;
}

export const TrendAnalysisChart: React.FC<TrendAnalysisChartProps> = ({
  ratios,
  statements,
  timeRange,
  onTimeRangeChange,
  selectedRatios = ['returnOnEquity'],
  onRatioSelectionChange,
}) => {
  const [chartType, setChartType] = useState<'line' | 'bar'>('line');
  const [showProjections, setShowProjections] = useState(false);
  const [isLoading] = useState(ratios.length === 0);
  const [selectedCategory, setSelectedCategory] = useState('profitability');
  const [selectedRatio, setSelectedRatio] = useState('returnOnEquity');
  const [timePeriodQuarters, setTimePeriodQuarters] = useState(12);

  // Group ratios by name and sort by period
  const ratioTrends = useMemo(() => {
    const grouped = ratios.reduce(
      (acc, ratio) => {
        if (!acc[ratio.ratioName]) {
          acc[ratio.ratioName] = [];
        }
        acc[ratio.ratioName].push({
          ...ratio,
          periodDate: new Date(ratio.periodEndDate),
        });
        return acc;
      },
      {} as Record<string, (FinancialRatio & { periodDate: Date })[]>
    );

    // Sort each ratio group by date
    Object.keys(grouped).forEach(ratioName => {
      grouped[ratioName].sort((a, b) => a.periodDate.getTime() - b.periodDate.getTime());
    });

    return grouped;
  }, [ratios]);

  // Get available ratios for selection
  const availableRatios = useMemo(() => {
    return Object.keys(ratioTrends).map(ratioName => {
      const firstRatio = ratioTrends[ratioName][0];
      return {
        name: ratioName,
        displayName: firstRatio?.ratioDisplayName || ratioName,
        category: firstRatio?.category || 'other',
      };
    });
  }, [ratioTrends]);

  // Filter ratios based on time range
  const filteredRatioTrends = useMemo(() => {
    const cutoffDate = new Date();
    switch (timeRange) {
      case '1Y':
        cutoffDate.setFullYear(cutoffDate.getFullYear() - 1);
        break;
      case '3Y':
        cutoffDate.setFullYear(cutoffDate.getFullYear() - 3);
        break;
      case '5Y':
        cutoffDate.setFullYear(cutoffDate.getFullYear() - 5);
        break;
      case '10Y':
        cutoffDate.setFullYear(cutoffDate.getFullYear() - 10);
        break;
    }

    const filtered: typeof ratioTrends = {};
    Object.keys(ratioTrends).forEach(ratioName => {
      const filteredData = ratioTrends[ratioName].filter(ratio => ratio.periodDate >= cutoffDate);
      if (filteredData.length > 0) {
        filtered[ratioName] = filteredData;
      }
    });

    return filtered;
  }, [ratioTrends, timeRange]);

  // Calculate trend direction and strength
  const calculateTrend = (
    ratioData: (FinancialRatio & { periodDate: Date })[]
  ): { direction: 'up' | 'down' | 'stable'; strength: number } => {
    if (ratioData.length < 2) return { direction: 'stable', strength: 0 };

    const first = ratioData[0].value;
    const last = ratioData[ratioData.length - 1].value;
    const change = last - first;
    const changePercent = (change / first) * 100;

    let direction: 'up' | 'down' | 'stable' = 'stable';
    let strength = Math.abs(changePercent);

    if (changePercent > 5) direction = 'up';
    else if (changePercent < -5) direction = 'down';

    return { direction, strength };
  };

  // Generate projection data (mock implementation)
  const generateProjections = (ratioData: (FinancialRatio & { periodDate: Date })[]) => {
    if (ratioData.length < 2) return [];

    const lastTwo = ratioData.slice(-2);
    const trend = lastTwo[1].value - lastTwo[0].value;
    const projections = [];

    for (let i = 1; i <= 4; i++) {
      const projectedDate = new Date(lastTwo[1].periodDate);
      projectedDate.setMonth(projectedDate.getMonth() + i * 3); // Quarterly projections

      const projectedValue = lastTwo[1].value + trend * i;
      projections.push({
        periodDate: projectedDate,
        value: projectedValue,
        isProjection: true,
      });
    }

    return projections;
  };

  const formatValue = (value: number, ratioName: string) => {
    // Determine if this is a percentage ratio
    const percentageRatios = ['returnOnEquity', 'returnOnAssets', 'grossMargin', 'netMargin'];
    if (percentageRatios.includes(ratioName)) {
      return `${(value * 100).toFixed(1)}%`;
    }
    return value.toFixed(2);
  };

  const getTrendIcon = (direction: 'up' | 'down' | 'stable') => {
    switch (direction) {
      case 'up':
        return <TrendingUp className='h-4 w-4 text-green-600' />;
      case 'down':
        return <TrendingDown className='h-4 w-4 text-red-600' />;
      default:
        return <div className='h-4 w-4 bg-gray-400 rounded-full' />;
    }
  };

  const getTrendColor = (direction: 'up' | 'down' | 'stable') => {
    switch (direction) {
      case 'up':
        return 'text-green-600';
      case 'down':
        return 'text-red-600';
      default:
        return 'text-gray-600';
    }
  };

  return (
    <div className='space-y-6'>
      {/* Controls */}
      <Card>
        <CardHeader>
          <div className='flex items-center justify-between'>
            <CardTitle className='flex items-center space-x-2'>
              <LineChart className='h-5 w-5' />
              <span>Financial Ratio Trends</span>
            </CardTitle>
            <div className='flex items-center space-x-2'>
              <Button
                variant={chartType === 'line' ? 'default' : 'outline'}
                size='sm'
                onClick={() => setChartType('line')}
              >
                <LineChart className='h-4 w-4 mr-2' />
                Line
              </Button>
              <Button
                variant={chartType === 'bar' ? 'default' : 'outline'}
                size='sm'
                onClick={() => setChartType('bar')}
              >
                <BarChart3 className='h-4 w-4 mr-2' />
                Bar
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className='space-y-4'>
            <div className='flex items-center space-x-4'>
              <div className='flex items-center space-x-2'>
                <Filter className='h-4 w-4' />
                <label className='text-sm font-medium' htmlFor='category-filter'>
                  Category:
                </label>
                <select
                  value={selectedCategory}
                  onChange={e => setSelectedCategory(e.target.value)}
                  className='w-32 p-2 border rounded-md'
                  id='category-filter'
                  aria-label='Select ratio category'
                >
                  <option value='profitability'>Profitability</option>
                  <option value='liquidity'>Liquidity</option>
                  <option value='leverage'>Leverage</option>
                </select>
              </div>

              <div className='flex items-center space-x-2'>
                <label className='text-sm font-medium' htmlFor='ratio-analyzer'>
                  Select Ratio to Analyze
                </label>
                <select
                  value={selectedRatio}
                  onChange={e => {
                    setSelectedRatio(e.target.value);
                    onRatioSelectionChange?.([e.target.value]);
                  }}
                  className='w-48 p-2 border rounded-md'
                  id='ratio-analyzer'
                  aria-label='Select financial ratio to analyze'
                >
                  <option value='all'>All Ratios</option>
                  <option value='returnOnEquity'>Return on Equity</option>
                  <option value='currentRatio'>Current Ratio</option>
                  <option value='debtToEquity'>Debt to Equity</option>
                </select>
              </div>
            </div>

            <div className='flex items-center space-x-4'>
              <div className='flex items-center space-x-2'>
                <Calendar className='h-4 w-4' />
                <span className='text-sm font-medium'>Time Range:</span>
                <Select value={timeRange} onValueChange={onTimeRangeChange}>
                  <SelectTrigger className='w-24'>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value='1Y'>1 Year</SelectItem>
                    <SelectItem value='3Y'>3 Years</SelectItem>
                    <SelectItem value='5Y'>5 Years</SelectItem>
                    <SelectItem value='10Y'>10 Years</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className='flex items-center space-x-2'>
                <span className='text-sm font-medium'>Time Period:</span>
                <select
                  value={timePeriodQuarters}
                  onChange={e => setTimePeriodQuarters(Number(e.target.value))}
                  className='w-20 p-1 border rounded-md text-sm'
                  aria-label='Select time period in quarters'
                >
                  <option value={4}>4</option>
                  <option value={8}>8</option>
                  <option value={12}>12</option>
                  <option value={16}>16</option>
                  <option value={24}>24</option>
                  <option value={36}>36</option>
                </select>
                <span className='text-xs text-gray-500'>quarters</span>
              </div>

              <Button
                variant='outline'
                size='sm'
                onClick={() => setShowProjections(!showProjections)}
              >
                {showProjections ? 'Hide' : 'Show'} Projections
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Ratio Trend Cards */}
      <div className='grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4'>
        {Object.entries(filteredRatioTrends).map(([ratioName, ratioData]) => {
          const trend = calculateTrend(ratioData);
          const firstValue = ratioData[0]?.value;
          const latestValue = ratioData[ratioData.length - 1]?.value;
          const change = latestValue - firstValue;
          const changePercent = firstValue ? (change / firstValue) * 100 : 0;

          return (
            <Card key={ratioName} className='hover:shadow-md transition-shadow'>
              <CardHeader className='pb-3'>
                <div className='flex items-center justify-between'>
                  <CardTitle className='text-lg'>
                    {ratioData[0]?.ratioDisplayName || ratioName}
                  </CardTitle>
                  <div className='flex items-center space-x-1'>
                    {getTrendIcon(trend.direction)}
                    <Badge variant='outline' className='text-xs'>
                      {ratioData[0]?.category}
                    </Badge>
                  </div>
                </div>
              </CardHeader>
              <CardContent>
                <div className='space-y-3'>
                  <div className='flex items-center justify-between'>
                    <span className='text-2xl font-bold'>
                      {latestValue ? formatValue(latestValue, ratioName) : '-'}
                    </span>
                    <span className={`text-sm ${getTrendColor(trend.direction)}`}>
                      {changePercent > 0 ? '+' : ''}
                      {changePercent.toFixed(1)}%
                    </span>
                  </div>

                  {/* Simple trend visualization */}
                  <div className='space-y-2'>
                    <div className='flex items-center justify-between text-sm'>
                      <span className='text-muted-foreground'>Trend:</span>
                      <span className={getTrendColor(trend.direction)}>
                        {trend.direction === 'up'
                          ? 'Improving'
                          : trend.direction === 'down'
                            ? 'Declining'
                            : 'Stable'}
                      </span>
                    </div>

                    <div className='flex items-center justify-between text-sm'>
                      <span className='text-muted-foreground'>Strength:</span>
                      <span>{Math.round(trend.strength * 100)}%</span>
                    </div>

                    <div className='flex items-center space-x-2'>
                      <div className='flex-1 bg-gray-200 rounded-full h-2'>
                        <div
                          className={`h-2 rounded-full ${
                            trend.direction === 'up'
                              ? 'bg-green-500'
                              : trend.direction === 'down'
                                ? 'bg-red-500'
                                : 'bg-gray-400'
                          }`}
                          style={{ width: `${Math.min(trend.strength * 2, 100)}%` }}
                        />
                      </div>
                      <span className='text-xs text-muted-foreground'>
                        {trend.strength.toFixed(1)}%
                      </span>
                    </div>
                  </div>

                  {/* Data points */}
                  <div className='space-y-1'>
                    <div className='flex items-center justify-between text-xs'>
                      <span className='text-muted-foreground'>Data Points:</span>
                      <span>{ratioData.length}</span>
                    </div>
                    <div className='flex items-center justify-between text-xs'>
                      <span className='text-muted-foreground'>Latest:</span>
                      <span>{ratioData[ratioData.length - 1]?.periodEndDate}</span>
                    </div>
                  </div>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {/* Chart Visualization Placeholder */}
      <Card>
        <CardHeader>
          <CardTitle>Trend Visualization</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='h-96 bg-gray-50 rounded-lg flex items-center justify-center'>
            <div className='text-center space-y-2'>
              <BarChart3 className='h-12 w-12 text-gray-400 mx-auto' />
              <p className='text-gray-600'>Interactive Chart Coming Soon</p>
              <p className='text-sm text-gray-500'>
                This will show {chartType} charts with trend lines and projections
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Projections Section */}
      {showProjections && (
        <Card>
          <CardHeader>
            <CardTitle>Forward-Looking Projections</CardTitle>
          </CardHeader>
          <CardContent>
            <div className='space-y-4'>
              {Object.entries(filteredRatioTrends)
                .slice(0, 3)
                .map(([ratioName, ratioData]) => {
                  const projections = generateProjections(ratioData);

                  return (
                    <div key={ratioName} className='p-4 border rounded-lg'>
                      <h4 className='font-medium mb-2'>
                        {ratioData[0]?.ratioDisplayName || ratioName}
                      </h4>
                      <div className='grid grid-cols-2 md:grid-cols-4 gap-4'>
                        {projections.map((projection, index) => (
                          <div key={index} className='text-center'>
                            <p className='text-sm text-muted-foreground'>
                              Q{Math.floor(projection.periodDate.getMonth() / 3) + 1}{' '}
                              {projection.periodDate.getFullYear()}
                            </p>
                            <p className='font-bold text-lg'>
                              {formatValue(projection.value, ratioName)}
                            </p>
                            <p className='text-xs text-muted-foreground'>Projected</p>
                          </div>
                        ))}
                      </div>
                    </div>
                  );
                })}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Chart Controls */}
      <Card>
        <CardHeader>
          <CardTitle>Chart Controls</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='flex items-center space-x-2'>
            <Button variant='outline' size='sm'>
              Zoom In
            </Button>
            <Button variant='outline' size='sm'>
              Zoom Out
            </Button>
            <Button variant='outline' size='sm'>
              Reset View
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Time Period Controls */}
      <Card>
        <CardHeader>
          <CardTitle>Time Period</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='space-y-2'>
            <select className='w-full p-2 border rounded-md' defaultValue='quarterly'>
              <option value='monthly'>Monthly</option>
              <option value='quarterly'>Quarterly</option>
              <option value='annually'>Annually</option>
            </select>
          </div>
        </CardContent>
      </Card>

      {/* Export Chart */}
      <Card>
        <CardHeader>
          <CardTitle>Export Chart</CardTitle>
        </CardHeader>
        <CardContent>
          <Button variant='outline' className='w-full'>
            <Download className='h-4 w-4 mr-2' />
            Export Chart
          </Button>
        </CardContent>
      </Card>

      {/* Data Quality Indicators */}
      <Card>
        <CardHeader>
          <CardTitle>Data Quality</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='space-y-2'>
            <p className='text-sm'>High quality financial data</p>
            <div className='text-xs text-muted-foreground'>95% data completeness</div>
          </div>
        </CardContent>
      </Card>

      {/* Benchmark Information */}
      <Card>
        <CardHeader>
          <CardTitle>Benchmark</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='space-y-2'>
            <p className='text-sm'>Industry benchmark comparison available</p>
            <Button variant='outline' size='sm'>
              View Benchmarks
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
