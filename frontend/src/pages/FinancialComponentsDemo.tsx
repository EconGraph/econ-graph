import React, { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import {
  Building2,
  Smartphone,
  Tablet,
  Monitor,
  Eye,
  Code,
  Palette,
  BarChart3,
  TrendingUp,
  AlertTriangle,
  Download,
  Users,
  DollarSign,
} from 'lucide-react';
import {
  FinancialDashboard,
  FinancialStatementViewer,
  TrendAnalysisChart,
  PeerComparisonChart,
  FinancialAlerts,
  FinancialExport,
  FinancialMobile,
  Company,
  FinancialStatement,
  FinancialRatio,
} from '@/components/financial';

// Mock data for demonstration
const mockCompany: Company = {
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
};

const mockStatements: FinancialStatement[] = [
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
    lineItems: [],
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
    lineItems: [],
    createdAt: '2023-09-30T00:00:00Z',
    updatedAt: '2023-09-30T00:00:00Z',
  },
];

const mockRatios: FinancialRatio[] = [
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
];

export const FinancialComponentsDemo: React.FC = () => {
  const [activeComponent, setActiveComponent] = useState('dashboard');
  const [deviceType, setDeviceType] = useState<'mobile' | 'tablet' | 'desktop'>('desktop');
  const [userType, setUserType] = useState<'beginner' | 'intermediate' | 'advanced' | 'expert'>('intermediate');

  const components = [
    {
      id: 'dashboard',
      name: 'Financial Dashboard',
      description: 'Comprehensive dashboard with company overview, key metrics, and navigation',
      icon: <Building2 className="h-5 w-5" />,
      component: (
        <FinancialDashboard
          companyId={mockCompany.id}
          userType={userType}
          showEducationalContent={true}
          showCollaborativeFeatures={true}
        />
      ),
    },
    {
      id: 'statement-viewer',
      name: 'Financial Statement Viewer',
      description: 'Detailed view of financial statements with annotations and analysis',
      icon: <DollarSign className="h-5 w-5" />,
      component: (
        <FinancialStatementViewer
          statementId={mockStatements[0].id}
          companyId={mockCompany.id}
          userType={userType}
          showEducationalContent={true}
          showCollaborativeFeatures={true}
        />
      ),
    },
    {
      id: 'trend-analysis',
      name: 'Trend Analysis Chart',
      description: 'Interactive charts showing financial trends over time',
      icon: <TrendingUp className="h-5 w-5" />,
      component: (
        <TrendAnalysisChart
          ratios={mockRatios}
          statements={mockStatements}
          timeRange="3Y"
          onTimeRangeChange={() => {}}
        />
      ),
    },
    {
      id: 'peer-comparison',
      name: 'Peer Comparison Chart',
      description: 'Compare financial metrics with industry peers',
      icon: <Users className="h-5 w-5" />,
      component: (
        <PeerComparisonChart
          ratios={mockRatios}
          company={mockCompany}
          userType={userType}
        />
      ),
    },
    {
      id: 'alerts',
      name: 'Financial Alerts',
      description: 'Real-time alerts and notifications for financial metrics',
      icon: <AlertTriangle className="h-5 w-5" />,
      component: (
        <FinancialAlerts
          companyId={mockCompany.id}
          ratios={mockRatios}
          statements={mockStatements}
          userType={userType}
        />
      ),
    },
    {
      id: 'export',
      name: 'Financial Export',
      description: 'Export financial data in various formats (PDF, Excel, CSV)',
      icon: <Download className="h-5 w-5" />,
      component: (
        <FinancialExport
          company={mockCompany}
          statements={mockStatements}
          ratios={mockRatios}
          userType={userType}
        />
      ),
    },
    {
      id: 'mobile',
      name: 'Mobile Components',
      description: 'Mobile-responsive financial components and layouts',
      icon: <Smartphone className="h-5 w-5" />,
      component: (
        <FinancialMobile
          company={mockCompany}
          statements={mockStatements}
          ratios={mockRatios}
          userType={userType}
          showEducationalContent={true}
          showCollaborativeFeatures={true}
        />
      ),
    },
  ];

  const currentComponent = components.find(c => c.id === activeComponent);

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Header */}
      <div className="bg-white border-b shadow-sm">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between h-16">
            <div className="flex items-center space-x-4">
              <div className="p-2 bg-blue-100 rounded-lg">
                <BarChart3 className="h-6 w-6 text-blue-600" />
              </div>
              <div>
                <h1 className="text-xl font-semibold">Financial Components Demo</h1>
                <p className="text-sm text-gray-500">
                  Interactive showcase of financial UI components
                </p>
              </div>
            </div>
            
            <div className="flex items-center space-x-4">
              <Select value={deviceType} onValueChange={(value: string) => setDeviceType(value as 'mobile' | 'tablet' | 'desktop')}>
                <SelectTrigger className="w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="desktop">
                    <div className="flex items-center space-x-2">
                      <Monitor className="h-4 w-4" />
                      <span>Desktop</span>
                    </div>
                  </SelectItem>
                  <SelectItem value="tablet">
                    <div className="flex items-center space-x-2">
                      <Tablet className="h-4 w-4" />
                      <span>Tablet</span>
                    </div>
                  </SelectItem>
                  <SelectItem value="mobile">
                    <div className="flex items-center space-x-2">
                      <Smartphone className="h-4 w-4" />
                      <span>Mobile</span>
                    </div>
                  </SelectItem>
                </SelectContent>
              </Select>

              <Select value={userType} onValueChange={(value: string) => setUserType(value as 'beginner' | 'intermediate' | 'advanced' | 'expert')}>
                <SelectTrigger className="w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="beginner">Beginner</SelectItem>
                  <SelectItem value="intermediate">Intermediate</SelectItem>
                  <SelectItem value="advanced">Advanced</SelectItem>
                  <SelectItem value="expert">Expert</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>
      </div>

      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-8">
          {/* Component Navigation */}
          <div className="lg:col-span-1">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center space-x-2">
                  <Code className="h-5 w-5" />
                  <span>Components</span>
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-2">
                  {components.map((component) => (
                    <Button
                      key={component.id}
                      variant={activeComponent === component.id ? 'default' : 'ghost'}
                      className="w-full justify-start"
                      onClick={() => setActiveComponent(component.id)}
                    >
                      <div className="flex items-center space-x-3">
                        {component.icon}
                        <div className="text-left">
                          <div className="font-medium">{component.name}</div>
                          <div className="text-xs text-muted-foreground">
                            {component.description}
                          </div>
                        </div>
                      </div>
                    </Button>
                  ))}
                </div>
              </CardContent>
            </Card>

            {/* Component Info */}
            {currentComponent && (
              <Card className="mt-4">
                <CardHeader>
                  <CardTitle className="flex items-center space-x-2">
                    <Eye className="h-5 w-5" />
                    <span>Component Info</span>
                  </CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-3">
                    <div>
                      <h3 className="font-medium">{currentComponent.name}</h3>
                      <p className="text-sm text-muted-foreground">
                        {currentComponent.description}
                      </p>
                    </div>
                    
                    <div className="space-y-2">
                      <div className="flex items-center justify-between text-sm">
                        <span>Device Type:</span>
                        <Badge variant="outline">{deviceType}</Badge>
                      </div>
                      <div className="flex items-center justify-between text-sm">
                        <span>User Type:</span>
                        <Badge variant="outline">{userType}</Badge>
                      </div>
                    </div>

                    <div className="pt-2 border-t">
                      <h4 className="font-medium text-sm mb-2">Features:</h4>
                      <ul className="text-sm text-muted-foreground space-y-1">
                        <li>• Responsive design</li>
                        <li>• Real-time updates</li>
                        <li>• Interactive charts</li>
                        <li>• Educational content</li>
                        <li>• Collaborative features</li>
                      </ul>
                    </div>
                  </div>
                </CardContent>
              </Card>
            )}
          </div>

          {/* Component Preview */}
          <div className="lg:col-span-3">
            <Card>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <CardTitle className="flex items-center space-x-2">
                    {currentComponent?.icon}
                    <span>{currentComponent?.name}</span>
                  </CardTitle>
                  <div className="flex items-center space-x-2">
                    <Badge variant="outline">{deviceType}</Badge>
                    <Badge variant="outline">{userType}</Badge>
                  </div>
                </div>
              </CardHeader>
              <CardContent>
                <div className="border rounded-lg bg-white">
                  {currentComponent?.component}
                </div>
              </CardContent>
            </Card>

            {/* Usage Instructions */}
            <Card className="mt-6">
              <CardHeader>
                <CardTitle className="flex items-center space-x-2">
                  <Palette className="h-5 w-5" />
                  <span>Usage Instructions</span>
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-4">
                  <div>
                    <h3 className="font-medium mb-2">Import Component:</h3>
                    <div className="bg-gray-100 p-3 rounded-lg font-mono text-sm">
                      {`import { ${currentComponent?.name} } from '@/components/financial';`}
                    </div>
                  </div>

                  <div>
                    <h3 className="font-medium mb-2">Basic Usage:</h3>
                    <div className="bg-gray-100 p-3 rounded-lg font-mono text-sm overflow-x-auto">
                      {`<${currentComponent?.name}
  companyId="${mockCompany.id}"
  userType="${userType}"
  showEducationalContent={true}
  showCollaborativeFeatures={true}
/>`}
                    </div>
                  </div>

                  <div>
                    <h3 className="font-medium mb-2">Props:</h3>
                    <ul className="text-sm space-y-1">
                      <li><code>companyId</code> - Company identifier</li>
                      <li><code>userType</code> - User experience level (beginner, intermediate, advanced, expert)</li>
                      <li><code>showEducationalContent</code> - Enable educational features</li>
                      <li><code>showCollaborativeFeatures</code> - Enable collaboration features</li>
                    </ul>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    </div>
  );
};
