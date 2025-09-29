import React, { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import {
  Badge,
  Button,
  Progress as _Progress,
  Alert,
  AlertDescription,
  Tooltip,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui';
import {
  TrendingUp,
  DollarSign,
  Calculator,
  BookOpen,
  Lightbulb,
  Info,
  Star,
  Target,
  Zap,
} from 'lucide-react';
// import { FinancialRatio } from '@/types/financial'; // Not used in current implementation
import { RatioExplanationModal } from './RatioExplanationModal';
import { BenchmarkComparison } from './BenchmarkComparison';
import { useSuspenseQuery } from '@tanstack/react-query';
import { executeGraphQL } from '../../utils/graphql';
import { GET_FINANCIAL_RATIOS } from '../../test-utils/mocks/graphql/ratio-queries';

interface RatioAnalysisPanelProps {
  statementId: string;
  userType: 'beginner' | 'intermediate' | 'advanced' | 'expert';
  showEducationalContent?: boolean;
}

export const RatioAnalysisPanel: React.FC<RatioAnalysisPanelProps> = ({
  statementId,
  userType,
  showEducationalContent = true,
}) => {
  const [selectedRatio, setSelectedRatio] = useState<string | null>(null);
  const [showExplanation, setShowExplanation] = useState(false);
  const [showBenchmarks, setShowBenchmarks] = useState(false);

  // GraphQL query for financial ratios - use Suspense for loading state
  const { data: ratiosData } = useSuspenseQuery({
    queryKey: ['financial-ratios', statementId],
    queryFn: async () => {
      const result = await executeGraphQL({
        query: GET_FINANCIAL_RATIOS,
        variables: { statementId },
      });
      return result.data;
    },
    staleTime: 5 * 60 * 1000, // 5 minutes
  });

  const ratios = ratiosData?.financialRatios;

  // Loading state is now handled by Suspense

  if (!ratios || ratios.length === 0) {
    return (
      <Alert>
        <AlertDescription>
          No ratio data available. Please ensure market data is provided for valuation ratios.
        </AlertDescription>
      </Alert>
    );
  }

  const getRatioCategory = (ratioName: string) => {
    const profitabilityRatios = [
      'returnOnEquity',
      'returnOnAssets',
      'returnOnInvestedCapital',
      'grossProfitMargin',
      'operatingProfitMargin',
      'netProfitMargin',
      'ebitdaMargin',
      'freeCashFlowMargin',
    ];
    const liquidityRatios = ['currentRatio', 'quickRatio', 'cashRatio', 'operatingCashFlowRatio'];
    const leverageRatios = [
      'debtToEquity',
      'debtToAssets',
      'interestCoverage',
      'debtServiceCoverage',
      'equityMultiplier',
    ];
    const valuationRatios = [
      'priceToEarnings',
      'priceToSales',
      'priceToBook',
      'pegRatio',
      'enterpriseValueToEbitda',
      'enterpriseValueToSales',
      'enterpriseValueToFreeCashFlow',
    ];
    const cashFlowRatios = [
      'freeCashFlow',
      'freeCashFlowPerShare',
      'freeCashFlowYield',
      'cashFlowReturnOnInvestment',
      'cashConversionCycle',
    ];
    const growthRatios = [
      'revenueGrowthRate',
      'earningsGrowthRate',
      'freeCashFlowGrowthRate',
      'bookValueGrowthRate',
    ];

    if (profitabilityRatios.includes(ratioName)) return 'profitability';
    if (liquidityRatios.includes(ratioName)) return 'liquidity';
    if (leverageRatios.includes(ratioName)) return 'leverage';
    if (valuationRatios.includes(ratioName)) return 'valuation';
    if (cashFlowRatios.includes(ratioName)) return 'cashFlow';
    if (growthRatios.includes(ratioName)) return 'growth';
    return 'other';
  };

  const getRatioIcon = (category: string) => {
    switch (category) {
      case 'profitability':
        return <TrendingUp className='h-4 w-4 text-green-600' />;
      case 'liquidity':
        return <DollarSign className='h-4 w-4 text-blue-600' />;
      case 'leverage':
        return <Target className='h-4 w-4 text-orange-600' />;
      case 'valuation':
        return <Calculator className='h-4 w-4 text-purple-600' />;
      case 'cashFlow':
        return <Zap className='h-4 w-4 text-yellow-600' />;
      case 'growth':
        return <TrendingUp className='h-4 w-4 text-indigo-600' />;
      default:
        return <Info className='h-4 w-4 text-gray-600' />;
    }
  };

  const getRatioColor = (value: number | null, ratioName: string) => {
    if (value === null) return 'text-muted-foreground';

    // Define thresholds for different ratios
    const thresholds: Record<string, { good: number; warning: number }> = {
      returnOnEquity: { good: 0.15, warning: 0.1 },
      currentRatio: { good: 2.0, warning: 1.5 },
      debtToEquity: { good: 0.5, warning: 1.0 },
      enterpriseValueToEbitda: { good: 15, warning: 25 },
      freeCashFlowYield: { good: 0.05, warning: 0.03 },
    };

    const threshold = thresholds[ratioName];
    if (!threshold) return 'text-foreground';

    if (ratioName === 'debtToEquity') {
      // Lower is better for debt ratios
      return value <= threshold.good
        ? 'text-green-600'
        : value <= threshold.warning
          ? 'text-yellow-600'
          : 'text-red-600';
    } else {
      // Higher is better for most ratios
      return value >= threshold.good
        ? 'text-green-600'
        : value >= threshold.warning
          ? 'text-yellow-600'
          : 'text-red-600';
    }
  };

  const formatRatioValue = (value: number | null, ratioName: string) => {
    if (value === null) return '-';

    if (
      ratioName.includes('Growth') ||
      ratioName.includes('Margin') ||
      ratioName.includes('Return') ||
      ratioName.includes('Yield')
    ) {
      return `${(value * 100).toFixed(1)}%`;
    }

    if (ratioName.includes('Ratio') || ratioName.includes('Coverage')) {
      return value.toFixed(2);
    }

    if (
      ratioName.includes('EV') ||
      ratioName.includes('P/E') ||
      ratioName.includes('P/S') ||
      ratioName.includes('P/B')
    ) {
      return `${value.toFixed(1)}x`;
    }

    if (ratioName.includes('FreeCashFlow') && !ratioName.includes('Yield')) {
      return new Intl.NumberFormat('en-US', {
        style: 'currency',
        currency: 'USD',
        minimumFractionDigits: 0,
        maximumFractionDigits: 0,
      }).format(value);
    }

    return value.toFixed(2);
  };

  const getRatioDisplayName = (ratioName: string) => {
    const displayNames: Record<string, string> = {
      returnOnEquity: 'Return on Equity (ROE)',
      returnOnAssets: 'Return on Assets (ROA)',
      returnOnInvestedCapital: 'Return on Invested Capital (ROIC)',
      grossProfitMargin: 'Gross Profit Margin',
      operatingProfitMargin: 'Operating Profit Margin',
      netProfitMargin: 'Net Profit Margin',
      ebitdaMargin: 'EBITDA Margin',
      freeCashFlowMargin: 'Free Cash Flow Margin',
      currentRatio: 'Current Ratio',
      quickRatio: 'Quick Ratio',
      cashRatio: 'Cash Ratio',
      operatingCashFlowRatio: 'Operating Cash Flow Ratio',
      debtToEquity: 'Debt-to-Equity Ratio',
      debtToAssets: 'Debt-to-Assets Ratio',
      interestCoverage: 'Interest Coverage Ratio',
      debtServiceCoverage: 'Debt Service Coverage Ratio',
      equityMultiplier: 'Equity Multiplier',
      priceToEarnings: 'Price-to-Earnings (P/E)',
      priceToSales: 'Price-to-Sales (P/S)',
      priceToBook: 'Price-to-Book (P/B)',
      pegRatio: 'PEG Ratio',
      enterpriseValueToEbitda: 'Enterprise Value to EBITDA (EV/EBITDA)',
      enterpriseValueToSales: 'Enterprise Value to Sales (EV/Sales)',
      enterpriseValueToFreeCashFlow: 'Enterprise Value to Free Cash Flow (EV/FCF)',
      freeCashFlow: 'Free Cash Flow',
      freeCashFlowPerShare: 'Free Cash Flow per Share',
      freeCashFlowYield: 'Free Cash Flow Yield',
      cashFlowReturnOnInvestment: 'Cash Flow Return on Investment (CFROI)',
      cashConversionCycle: 'Cash Conversion Cycle',
      revenueGrowthRate: 'Revenue Growth Rate',
      earningsGrowthRate: 'Earnings Growth Rate',
      freeCashFlowGrowthRate: 'Free Cash Flow Growth Rate',
      bookValueGrowthRate: 'Book Value Growth Rate',
    };

    return displayNames[ratioName] || ratioName;
  };

  const getRatioImportance = (ratioName: string) => {
    // Warren Buffett favorites and analyst preferred ratios
    const warrenBuffettFavorites = [
      'freeCashFlow',
      'freeCashFlowYield',
      'returnOnEquity',
      'returnOnInvestedCapital',
    ];
    const analystPreferred = [
      'enterpriseValueToEbitda',
      'enterpriseValueToSales',
      'enterpriseValueToFreeCashFlow',
      'returnOnInvestedCapital',
    ];

    if (warrenBuffettFavorites.includes(ratioName)) {
      return {
        level: 'warren',
        label: 'Warren Buffett Favorite',
        icon: <Star className='h-3 w-3' />,
      };
    }
    if (analystPreferred.includes(ratioName)) {
      return { level: 'analyst', label: 'Analyst Preferred', icon: <Target className='h-3 w-3' /> };
    }
    return null;
  };

  const renderRatioRow = (ratioName: string, value: number | null) => {
    const category = getRatioCategory(ratioName);
    const importance = getRatioImportance(ratioName);
    const displayName = getRatioDisplayName(ratioName);

    return (
      <TableRow key={ratioName} className='hover:bg-muted/50'>
        <TableCell>
          <div className='flex items-center space-x-2'>
            {getRatioIcon(category)}
            <span className='font-medium'>{displayName}</span>
            {importance && (
              <TooltipProvider>
                <Tooltip
                  content={
                    importance.level === 'warren'
                      ? "Warren Buffett's preferred metric for value investing"
                      : 'Preferred by professional analysts for better comparability'
                  }
                >
                  <TooltipTrigger>
                    <Badge variant='outline' className='text-xs'>
                      {importance.icon}
                      <span className='ml-1'>{importance.label}</span>
                    </Badge>
                  </TooltipTrigger>
                </Tooltip>
              </TooltipProvider>
            )}
          </div>
        </TableCell>
        <TableCell>
          <span className={`font-mono text-lg ${getRatioColor(value, ratioName)}`}>
            {formatRatioValue(value, ratioName)}
          </span>
        </TableCell>
        <TableCell>
          <div className='flex items-center space-x-2'>
            {showEducationalContent && (
              <Button
                variant='ghost'
                size='sm'
                onClick={() => {
                  setSelectedRatio(ratioName);
                  setShowExplanation(true);
                }}
              >
                <BookOpen className='h-4 w-4' />
              </Button>
            )}
            <Button
              variant='ghost'
              size='sm'
              onClick={() => {
                setSelectedRatio(ratioName);
                setShowBenchmarks(true);
              }}
            >
              <Target className='h-4 w-4' />
            </Button>
          </div>
        </TableCell>
      </TableRow>
    );
  };

  const getRatiosByCategory = () => {
    if (!ratios || !Array.isArray(ratios)) {
      return {
        profitability: [],
        liquidity: [],
        leverage: [],
        valuation: [],
        cashFlow: [],
        growth: [],
      };
    }

    const categories: Record<string, Array<[string, number | null]>> = {
      profitability: [],
      liquidity: [],
      leverage: [],
      valuation: [],
      cashFlow: [],
      growth: [],
    };

    ratios.forEach((ratio: any) => {
      const category = getRatioCategory(ratio.ratioName);
      if (categories[category]) {
        categories[category].push([ratio.ratioName, ratio.value]);
      }
    });

    return categories;
  };

  const categories = getRatiosByCategory();

  return (
    <div className='space-y-6'>
      {/* Header with key insights */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center space-x-2'>
            <Calculator className='h-5 w-5' />
            <span>Financial Ratio Analysis</span>
          </CardTitle>
          <div className='grid grid-cols-1 md:grid-cols-3 gap-4 mt-4'>
            {/* Key metrics summary */}
            <div className='p-4 border rounded-lg'>
              <h3 className='font-semibold text-green-600'>Profitability</h3>
              <p className='text-2xl font-bold'>
                {ratios?.find((r: any) => r.ratioName === 'returnOnEquity')?.value
                  ? `${(ratios.find((r: any) => r.ratioName === 'returnOnEquity')!.value * 100).toFixed(1)}%`
                  : '-'}
              </p>
              <p className='text-sm text-muted-foreground'>Return on Equity</p>
            </div>

            <div className='p-4 border rounded-lg'>
              <h3 className='font-semibold text-blue-600'>Liquidity</h3>
              <p className='text-2xl font-bold'>
                {ratios?.find((r: any) => r.ratioName === 'currentRatio')?.value
                  ? ratios.find((r: any) => r.ratioName === 'currentRatio')!.value.toFixed(2)
                  : '-'}
              </p>
              <p className='text-sm text-muted-foreground'>Current Ratio</p>
            </div>

            <div className='p-4 border rounded-lg'>
              <h3 className='font-semibold text-purple-600'>Valuation</h3>
              <p className='text-2xl font-bold'>
                {ratios?.find((r: any) => r.ratioName === 'enterpriseValueToEbitda')?.value
                  ? `${ratios.find((r: any) => r.ratioName === 'enterpriseValueToEbitda')!.value.toFixed(1)}x`
                  : '-'}
              </p>
              <p className='text-sm text-muted-foreground'>EV/EBITDA</p>
            </div>
          </div>
        </CardHeader>
      </Card>

      {/* Detailed ratios by category */}
      <Tabs defaultValue='profitability' className='space-y-4'>
        <TabsList className='grid w-full grid-cols-6'>
          <TabsTrigger value='profitability'>Profitability</TabsTrigger>
          <TabsTrigger value='liquidity'>Liquidity</TabsTrigger>
          <TabsTrigger value='leverage'>Leverage</TabsTrigger>
          <TabsTrigger value='valuation'>Valuation</TabsTrigger>
          <TabsTrigger value='cashFlow'>Cash Flow</TabsTrigger>
          <TabsTrigger value='growth'>Growth</TabsTrigger>
        </TabsList>

        {Object.entries(categories).map(([category, ratioList]) => (
          <TabsContent key={category} value={category}>
            <Card>
              <CardHeader>
                <CardTitle className='flex items-center space-x-2'>
                  {getRatioIcon(category)}
                  <span className='capitalize'>{category} Ratios</span>
                </CardTitle>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Ratio</TableHead>
                      <TableHead>Value</TableHead>
                      <TableHead>Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {ratioList.map(([ratioName, value]) => renderRatioRow(ratioName, value))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </TabsContent>
        ))}
      </Tabs>

      {/* Educational content for beginners */}
      {userType === 'beginner' && showEducationalContent && (
        <Card>
          <CardHeader>
            <CardTitle className='flex items-center space-x-2'>
              <Lightbulb className='h-5 w-5' />
              <span>Understanding Financial Ratios</span>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className='space-y-4'>
              <Alert>
                <Info className='h-4 w-4' />
                <AlertDescription>
                  <strong>Start with these key ratios:</strong>
                  <ul className='mt-2 space-y-1'>
                    <li>
                      • <strong>Return on Equity (ROE)</strong>: How efficiently the company uses
                      shareholder money
                    </li>
                    <li>
                      • <strong>Current Ratio</strong>: Company's ability to pay short-term debts
                    </li>
                    <li>
                      • <strong>Free Cash Flow</strong>: Warren Buffett's favorite metric - actual
                      cash generated
                    </li>
                  </ul>
                </AlertDescription>
              </Alert>

              <div className='grid grid-cols-1 md:grid-cols-2 gap-4'>
                <div className='p-4 border rounded-lg'>
                  <h4 className='font-semibold text-green-600'>Warren Buffett Favorites</h4>
                  <p className='text-sm text-muted-foreground mt-1'>
                    Focus on Free Cash Flow, ROE, and ROIC. These metrics show how well a company
                    generates cash and uses capital efficiently.
                  </p>
                </div>

                <div className='p-4 border rounded-lg'>
                  <h4 className='font-semibold text-purple-600'>Analyst Preferred</h4>
                  <p className='text-sm text-muted-foreground mt-1'>
                    EV/EBITDA is preferred over P/E ratios because it eliminates differences in
                    capital structure and provides better comparability.
                  </p>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Modals */}
      {showExplanation && selectedRatio && (
        <RatioExplanationModal
          ratioName={selectedRatio}
          isOpen={showExplanation}
          onClose={() => setShowExplanation(false)}
          formula='Formula not available'
          description='Description not available'
          interpretation='Interpretation not available'
        />
      )}

      {showBenchmarks && selectedRatio && (
        <BenchmarkComparison
          ratioName={selectedRatio}
          companyValue={ratios?.find((r: any) => r.ratioName === selectedRatio)?.value || 0}
        />
      )}
    </div>
  );
};
