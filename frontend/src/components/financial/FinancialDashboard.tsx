import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Badge, Button, Progress, Alert, AlertDescription } from '@/components/ui';
import {
  TrendingUp,
  TrendingDown,
  DollarSign,
  Building2,
  Calendar,
  FileText,
  BarChart3,
  PieChart,
  Activity,
  AlertTriangle,
  CheckCircle,
  XCircle,
  RefreshCw,
  Download,
  Share2,
  Star,
  Eye,
  Filter,
  Search,
} from 'lucide-react';
import {
  GET_FINANCIAL_STATEMENTS,
  GET_FINANCIAL_RATIOS,
  GET_COMPANY_INFO,
} from '@/graphql/financial';
import { FinancialStatement, FinancialRatio, Company } from '@/types/financial';
import { FinancialStatementViewer } from './FinancialStatementViewer';
import { RatioAnalysisPanel } from './RatioAnalysisPanel';
import { BenchmarkComparison } from './BenchmarkComparison';
import { TrendAnalysisChart } from './TrendAnalysisChart';
import { PeerComparisonChart } from './PeerComparisonChart';

// Mock Apollo Client hooks for now
const useQuery = (query: any, options?: any) => ({
  data: {
    financialStatements: [
      {
        id: 'mock-statement-1',
        companyId: 'mock-company-id',
        filingType: '10-K',
        formType: '10-K',
        accessionNumber: '0001234567-23-000001',
        filingDate: '2023-12-31',
        periodEndDate: '2023-12-31',
        fiscalYear: 2023,
        fiscalQuarter: 4,
        documentType: 'XBRL',
        documentUrl: 'http://example.com/filing.xbrl',
        xbrlProcessingStatus: 'completed',
        isAmended: false,
        isRestated: false,
        createdAt: '2023-12-31T00:00:00Z',
        updatedAt: '2023-12-31T00:00:00Z',
      },
      {
        id: 'mock-statement-2',
        companyId: 'mock-company-id',
        filingType: '10-Q',
        formType: '10-Q',
        accessionNumber: '0001234567-23-000002',
        filingDate: '2023-09-30',
        periodEndDate: '2023-09-30',
        fiscalYear: 2023,
        fiscalQuarter: 3,
        documentType: 'XBRL',
        documentUrl: 'http://example.com/filing.xbrl',
        xbrlProcessingStatus: 'completed',
        isAmended: false,
        isRestated: false,
        createdAt: '2023-09-30T00:00:00Z',
        updatedAt: '2023-09-30T00:00:00Z',
      },
    ],
    company: {
      id: 'mock-company-id',
      cik: '0000320193',
      name: 'Apple Inc.',
      ticker: 'AAPL',
      sic: '3571',
      sicDescription: 'Electronic Computers',
      gics: '4520',
      gicsDescription: 'Technology Hardware & Equipment',
      businessStatus: 'active',
      fiscalYearEnd: '09-30',
      createdAt: '2023-01-01T00:00:00Z',
      updatedAt: '2023-12-31T00:00:00Z',
    },
    financialRatios: [
      {
        id: 'ratio-1',
        statementId: 'mock-statement-1',
        ratioName: 'returnOnEquity',
        ratioDisplayName: 'Return on Equity',
        value: 0.147,
        category: 'profitability',
        formula: 'Net Income / Shareholders Equity',
        interpretation: 'Strong profitability, above industry average',
        benchmarkPercentile: 75,
        periodEndDate: '2023-12-31',
        fiscalYear: 2023,
        fiscalQuarter: 4,
        calculatedAt: '2023-12-31T00:00:00Z',
        dataQualityScore: 0.95,
      },
      {
        id: 'ratio-2',
        statementId: 'mock-statement-1',
        ratioName: 'currentRatio',
        ratioDisplayName: 'Current Ratio',
        value: 1.04,
        category: 'liquidity',
        formula: 'Current Assets / Current Liabilities',
        interpretation: 'Adequate liquidity position',
        benchmarkPercentile: 45,
        periodEndDate: '2023-12-31',
        fiscalYear: 2023,
        fiscalQuarter: 4,
        calculatedAt: '2023-12-31T00:00:00Z',
        dataQualityScore: 0.98,
      },
      {
        id: 'ratio-3',
        statementId: 'mock-statement-1',
        ratioName: 'debtToEquity',
        ratioDisplayName: 'Debt to Equity',
        value: 1.73,
        category: 'leverage',
        formula: 'Total Debt / Shareholders Equity',
        interpretation: 'Moderate leverage, manageable debt levels',
        benchmarkPercentile: 60,
        periodEndDate: '2023-12-31',
        fiscalYear: 2023,
        fiscalQuarter: 4,
        calculatedAt: '2023-12-31T00:00:00Z',
        dataQualityScore: 0.92,
      },
    ],
  },
  loading: false,
  error: null,
  refetch: async () => Promise.resolve(),
});

interface FinancialDashboardProps {
  companyId: string;
  userType?: 'beginner' | 'intermediate' | 'advanced' | 'expert';
  showEducationalContent?: boolean;
  showCollaborativeFeatures?: boolean;
}

export const FinancialDashboard: React.FC<FinancialDashboardProps> = ({
  companyId,
  userType = 'intermediate',
  showEducationalContent = true,
  showCollaborativeFeatures = true,
}) => {
  const [activeTab, setActiveTab] = useState('overview');
  const [selectedStatement, setSelectedStatement] = useState<string | null>(null);
  const [timeRange, setTimeRange] = useState<'1Y' | '3Y' | '5Y' | '10Y'>('3Y');
  const [refreshKey, setRefreshKey] = useState(0);

  // GraphQL queries
  const {
    data,
    loading,
    error,
    refetch,
  } = useQuery(GET_FINANCIAL_STATEMENTS, {
    variables: { companyId, limit: 10 },
    fetchPolicy: 'cache-and-network',
  });

  const company: Company | undefined = data?.company;
  const statements: FinancialStatement[] = data?.financialStatements || [];
  const ratios: FinancialRatio[] = data?.financialRatios || [];

  // Auto-select the most recent statement
  useEffect(() => {
    if (statements.length > 0 && !selectedStatement) {
      setSelectedStatement(statements[0].id);
    }
  }, [statements, selectedStatement]);

  const handleRefresh = async () => {
    setRefreshKey(prev => prev + 1);
    await refetch();
  };

  const handleExportData = () => {
    // Implementation for data export
    console.log('Exporting financial data...');
  };

  const handleShareAnalysis = () => {
    // Implementation for sharing analysis
    console.log('Sharing financial analysis...');
  };

  const getStatusColor = (status: string) => {
    switch (status.toLowerCase()) {
      case 'completed':
        return 'text-green-600';
      case 'processing':
        return 'text-yellow-600';
      case 'failed':
        return 'text-red-600';
      default:
        return 'text-gray-600';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status.toLowerCase()) {
      case 'completed':
        return <CheckCircle className="h-4 w-4" />;
      case 'processing':
        return <RefreshCw className="h-4 w-4 animate-spin" />;
      case 'failed':
        return <XCircle className="h-4 w-4" />;
      default:
        return <AlertTriangle className="h-4 w-4" />;
    }
  };

  const formatCurrency = (value: number) => {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      minimumFractionDigits: 0,
      maximumFractionDigits: 0,
    }).format(value);
  };

  const formatPercent = (value: number) => {
    return `${(value * 100).toFixed(1)}%`;
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-8">
        <Progress value={33} className="w-full max-w-md" />
        <span className="ml-4">Loading financial dashboard...</span>
      </div>
    );
  }

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertDescription>
          Error loading financial data: {String(error)}
        </AlertDescription>
      </Alert>
    );
  }

  if (!company) {
    return (
      <Alert>
        <AlertDescription>Company not found.</AlertDescription>
      </Alert>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-4">
              <div className="p-2 bg-blue-100 rounded-lg">
                <Building2 className="h-6 w-6 text-blue-600" />
              </div>
              <div>
                <CardTitle className="text-2xl">{company.name}</CardTitle>
                <div className="flex items-center space-x-4 text-sm text-muted-foreground">
                  <span className="flex items-center space-x-1">
                    <span>Ticker:</span>
                    <Badge variant="outline">{company.ticker}</Badge>
                  </span>
                  <span className="flex items-center space-x-1">
                    <span>CIK:</span>
                    <span className="font-mono">{company.cik}</span>
                  </span>
                  <span className="flex items-center space-x-1">
                    <span>Industry:</span>
                    <span>{company.gicsDescription}</span>
                  </span>
                </div>
              </div>
            </div>

            <div className="flex items-center space-x-2">
              <Button variant="outline" size="sm" onClick={handleRefresh}>
                <RefreshCw className="h-4 w-4 mr-2" />
                Refresh
              </Button>
              <Button variant="outline" size="sm" onClick={handleExportData}>
                <Download className="h-4 w-4 mr-2" />
                Export
              </Button>
              <Button variant="outline" size="sm" onClick={handleShareAnalysis}>
                <Share2 className="h-4 w-4 mr-2" />
                Share
              </Button>
            </div>
          </div>
        </CardHeader>
      </Card>

      {/* Key Metrics Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-muted-foreground">Latest Filing</p>
                <p className="text-2xl font-bold">
                  {statements[0]?.formType || '-'}
                </p>
              </div>
              <FileText className="h-8 w-8 text-blue-600" />
            </div>
            <div className="flex items-center space-x-2 mt-2">
              {statements[0] && (
                <>
                  {getStatusIcon(statements[0].xbrlProcessingStatus)}
                  <span className={`text-sm ${getStatusColor(statements[0].xbrlProcessingStatus)}`}>
                    {statements[0].xbrlProcessingStatus}
                  </span>
                </>
              )}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-muted-foreground">ROE</p>
                <p className="text-2xl font-bold text-green-600">
                  {ratios.find(r => r.ratioName === 'returnOnEquity')?.value
                    ? formatPercent(ratios.find(r => r.ratioName === 'returnOnEquity')!.value)
                    : '-'}
                </p>
              </div>
              <TrendingUp className="h-8 w-8 text-green-600" />
            </div>
            <p className="text-sm text-muted-foreground mt-2">
              {ratios.find(r => r.ratioName === 'returnOnEquity')?.benchmarkPercentile
                ? `${ratios.find(r => r.ratioName === 'returnOnEquity')!.benchmarkPercentile}th percentile`
                : 'Return on Equity'}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-muted-foreground">Current Ratio</p>
                <p className="text-2xl font-bold text-blue-600">
                  {ratios.find(r => r.ratioName === 'currentRatio')?.value
                    ? ratios.find(r => r.ratioName === 'currentRatio')!.value.toFixed(2)
                    : '-'}
                </p>
              </div>
              <Activity className="h-8 w-8 text-blue-600" />
            </div>
            <p className="text-sm text-muted-foreground mt-2">
              {ratios.find(r => r.ratioName === 'currentRatio')?.benchmarkPercentile
                ? `${ratios.find(r => r.ratioName === 'currentRatio')!.benchmarkPercentile}th percentile`
                : 'Liquidity Position'}
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardContent className="p-6">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-muted-foreground">Debt/Equity</p>
                <p className="text-2xl font-bold text-purple-600">
                  {ratios.find(r => r.ratioName === 'debtToEquity')?.value
                    ? ratios.find(r => r.ratioName === 'debtToEquity')!.value.toFixed(2)
                    : '-'}
                </p>
              </div>
              <BarChart3 className="h-8 w-8 text-purple-600" />
            </div>
            <p className="text-sm text-muted-foreground mt-2">
              {ratios.find(r => r.ratioName === 'debtToEquity')?.benchmarkPercentile
                ? `${ratios.find(r => r.ratioName === 'debtToEquity')!.benchmarkPercentile}th percentile`
                : 'Leverage Ratio'}
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Main Content Tabs */}
      <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-4">
        <TabsList className="grid w-full grid-cols-6">
          <TabsTrigger value="overview" className="flex items-center space-x-2">
            <Eye className="h-4 w-4" />
            <span>Overview</span>
          </TabsTrigger>
          <TabsTrigger value="statements" className="flex items-center space-x-2">
            <FileText className="h-4 w-4" />
            <span>Statements</span>
          </TabsTrigger>
          <TabsTrigger value="ratios" className="flex items-center space-x-2">
            <BarChart3 className="h-4 w-4" />
            <span>Ratios</span>
          </TabsTrigger>
          <TabsTrigger value="trends" className="flex items-center space-x-2">
            <TrendingUp className="h-4 w-4" />
            <span>Trends</span>
          </TabsTrigger>
          <TabsTrigger value="comparison" className="flex items-center space-x-2">
            <PieChart className="h-4 w-4" />
            <span>Compare</span>
          </TabsTrigger>
          <TabsTrigger value="analysis" className="flex items-center space-x-2">
            <Activity className="h-4 w-4" />
            <span>Analysis</span>
          </TabsTrigger>
        </TabsList>

        {/* Overview Tab */}
        <TabsContent value="overview" className="space-y-4">
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <Card>
              <CardHeader>
                <CardTitle>Recent Filings</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-3">
                  {statements.slice(0, 5).map((statement) => (
                    <div
                      key={statement.id}
                      className="flex items-center justify-between p-3 border rounded-lg cursor-pointer hover:bg-muted/50"
                      onClick={() => setSelectedStatement(statement.id)}
                    >
                      <div className="flex items-center space-x-3">
                        <div className="p-2 bg-blue-100 rounded">
                          <FileText className="h-4 w-4 text-blue-600" />
                        </div>
                        <div>
                          <p className="font-medium">{statement.formType}</p>
                          <p className="text-sm text-muted-foreground">
                            FY {statement.fiscalYear} Q{statement.fiscalQuarter}
                          </p>
                        </div>
                      </div>
                      <div className="flex items-center space-x-2">
                        {getStatusIcon(statement.xbrlProcessingStatus)}
                        <span className="text-sm">{statement.periodEndDate}</span>
                      </div>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Key Financial Metrics</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-4">
                  {ratios.slice(0, 6).map((ratio) => (
                    <div key={ratio.id} className="flex items-center justify-between">
                      <div>
                        <p className="font-medium">{ratio.ratioDisplayName}</p>
                        <p className="text-sm text-muted-foreground">{ratio.category}</p>
                      </div>
                      <div className="text-right">
                        <p className="font-bold">
                          {ratio.value ? formatPercent(ratio.value) : '-'}
                        </p>
                        {ratio.benchmarkPercentile && (
                          <Badge variant="outline" className="text-xs">
                            {ratio.benchmarkPercentile}th percentile
                          </Badge>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        {/* Statements Tab */}
        <TabsContent value="statements" className="space-y-4">
          {selectedStatement ? (
            <FinancialStatementViewer
              statementId={selectedStatement}
              companyId={companyId}
              userType={userType}
              showEducationalContent={showEducationalContent}
              showCollaborativeFeatures={showCollaborativeFeatures}
            />
          ) : (
            <Alert>
              <AlertDescription>
                Please select a financial statement to view details.
              </AlertDescription>
            </Alert>
          )}
        </TabsContent>

        {/* Ratios Tab */}
        <TabsContent value="ratios">
          <RatioAnalysisPanel
            ratios={ratios}
            loading={false}
            userType={userType}
            showEducationalContent={showEducationalContent}
          />
        </TabsContent>

        {/* Trends Tab */}
        <TabsContent value="trends">
          <TrendAnalysisChart
            ratios={ratios}
            statements={statements}
            timeRange={timeRange}
            onTimeRangeChange={setTimeRange}
          />
        </TabsContent>

        {/* Comparison Tab */}
        <TabsContent value="comparison">
          <PeerComparisonChart
            ratios={ratios}
            company={company}
            userType={userType}
          />
        </TabsContent>

        {/* Analysis Tab */}
        <TabsContent value="analysis">
          <Card>
            <CardHeader>
              <CardTitle>Benchmark Analysis</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                {ratios.slice(0, 3).map((ratio) => (
                  <BenchmarkComparison
                    key={ratio.id}
                    ratioName={ratio.ratioDisplayName}
                    companyValue={ratio.value}
                    benchmarkData={{
                      percentile: ratio.benchmarkPercentile || 50,
                      industryMedian: 0.5,
                      industryP25: 0.25,
                      industryP75: 0.75,
                      industryP90: 0.9,
                      industryP10: 0.1,
                    }}
                  />
                ))}
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
};
