import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Drawer, DrawerContent, DrawerHeader, DrawerTitle, DrawerTrigger } from '@/components/ui/drawer';
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetTrigger } from '@/components/ui/sheet';
import { Progress } from '@/components/ui/progress';
import { Alert, AlertDescription } from '@/components/ui/alert';
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
  Menu,
  MoreVertical,
  ChevronRight,
  ChevronDown,
  Phone,
  Tablet,
  Monitor,
  Clock,
} from 'lucide-react';
import { FinancialStatement, FinancialRatio, Company } from '@/types/financial';
import { FinancialDashboard } from './FinancialDashboard';
import { FinancialStatementViewer } from './FinancialStatementViewer';
import { RatioAnalysisPanel } from './RatioAnalysisPanel';
import { TrendAnalysisChart } from './TrendAnalysisChart';
import { PeerComparisonChart } from './PeerComparisonChart';
import { FinancialAlerts } from './FinancialAlerts';
import { FinancialExport } from './FinancialExport';

interface FinancialMobileProps {
  company: Company;
  statements: FinancialStatement[];
  ratios: FinancialRatio[];
  userType?: 'beginner' | 'intermediate' | 'advanced' | 'expert';
  showEducationalContent?: boolean;
  showCollaborativeFeatures?: boolean;
}

export const FinancialMobile: React.FC<FinancialMobileProps> = ({
  company,
  statements,
  ratios,
  userType = 'intermediate',
  showEducationalContent = true,
  showCollaborativeFeatures = true,
}) => {
  const [activeTab, setActiveTab] = useState('overview');
  const [selectedStatement, setSelectedStatement] = useState<string | null>(null);
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const [deviceType, setDeviceType] = useState<'mobile' | 'tablet' | 'desktop'>('mobile');
  const [orientation, setOrientation] = useState<'portrait' | 'landscape'>('portrait');

  // Detect device type and orientation
  useEffect(() => {
    const detectDevice = () => {
      const width = window.innerWidth;
      const height = window.innerHeight;
      
      if (width < 768) {
        setDeviceType('mobile');
      } else if (width < 1024) {
        setDeviceType('tablet');
      } else {
        setDeviceType('desktop');
      }
      
      setOrientation(height > width ? 'portrait' : 'landscape');
    };

    detectDevice();
    window.addEventListener('resize', detectDevice);
    return () => window.removeEventListener('resize', detectDevice);
  }, []);

  // Auto-select the most recent statement
  useEffect(() => {
    if (statements.length > 0 && !selectedStatement) {
      setSelectedStatement(statements[0].id);
    }
  }, [statements, selectedStatement]);

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

  // Mobile-optimized key metrics
  const keyMetrics = [
    {
      title: 'ROE',
      value: ratios.find(r => r.ratioName === 'returnOnEquity')?.value
        ? formatPercent(ratios.find(r => r.ratioName === 'returnOnEquity')!.value)
        : '-',
      change: '+2.1%',
      trend: 'up',
      icon: TrendingUp,
    },
    {
      title: 'Current Ratio',
      value: ratios.find(r => r.ratioName === 'currentRatio')?.value
        ? ratios.find(r => r.ratioName === 'currentRatio')!.value.toFixed(2)
        : '-',
      change: '-0.1',
      trend: 'down',
      icon: TrendingDown,
    },
    {
      title: 'Debt/Equity',
      value: ratios.find(r => r.ratioName === 'debtToEquity')?.value
        ? ratios.find(r => r.ratioName === 'debtToEquity')!.value.toFixed(2)
        : '-',
      change: '+0.05',
      trend: 'up',
      icon: Activity,
    },
  ];

  // Mobile navigation tabs
  const mobileTabs = [
    { id: 'overview', label: 'Overview', icon: Eye },
    { id: 'statements', label: 'Statements', icon: FileText },
    { id: 'ratios', label: 'Ratios', icon: BarChart3 },
    { id: 'trends', label: 'Trends', icon: TrendingUp },
    { id: 'alerts', label: 'Alerts', icon: AlertTriangle },
  ];

  // Render mobile-optimized content
  if (deviceType === 'mobile') {
    return (
      <div className="min-h-screen bg-gray-50">
        {/* Mobile Header */}
        <div className="sticky top-0 z-50 bg-white border-b shadow-sm">
          <div className="flex items-center justify-between p-4">
            <div className="flex items-center space-x-3">
              <div className="p-2 bg-blue-100 rounded-lg">
                <Building2 className="h-5 w-5 text-blue-600" />
              </div>
              <div>
                <h1 className="font-semibold text-lg">{company.name}</h1>
                <p className="text-sm text-gray-500">{company.ticker}</p>
              </div>
            </div>
            <Sheet open={isMenuOpen} onOpenChange={setIsMenuOpen}>
              <SheetTrigger asChild>
                <Button variant="ghost" size="sm">
                  <Menu className="h-5 w-5" />
                </Button>
              </SheetTrigger>
              <SheetContent side="right" className="w-80">
                <SheetHeader>
                  <SheetTitle>Financial Analysis</SheetTitle>
                </SheetHeader>
                <div className="mt-6 space-y-4">
                  <div className="space-y-2">
                    <h3 className="font-medium">Quick Actions</h3>
                    <Button variant="outline" className="w-full justify-start">
                      <Download className="h-4 w-4 mr-2" />
                      Export Data
                    </Button>
                    <Button variant="outline" className="w-full justify-start">
                      <Share2 className="h-4 w-4 mr-2" />
                      Share Analysis
                    </Button>
                  </div>
                  <div className="space-y-2">
                    <h3 className="font-medium">Navigation</h3>
                    {mobileTabs.map((tab) => (
                      <Button
                        key={tab.id}
                        variant={activeTab === tab.id ? 'default' : 'ghost'}
                        className="w-full justify-start"
                        onClick={() => {
                          setActiveTab(tab.id);
                          setIsMenuOpen(false);
                        }}
                      >
                        <tab.icon className="h-4 w-4 mr-2" />
                        {tab.label}
                      </Button>
                    ))}
                  </div>
                </div>
              </SheetContent>
            </Sheet>
          </div>
        </div>

        {/* Mobile Content */}
        <div className="p-4 space-y-4">
          {/* Key Metrics Cards */}
          {activeTab === 'overview' && (
            <>
              <div className="space-y-3">
                {keyMetrics.map((metric, index) => (
                  <Card key={index}>
                    <CardContent className="p-4">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center space-x-3">
                          <div className={`p-2 rounded-lg ${
                            metric.trend === 'up' ? 'bg-green-100' : 'bg-red-100'
                          }`}>
                            <metric.icon className={`h-4 w-4 ${
                              metric.trend === 'up' ? 'text-green-600' : 'text-red-600'
                            }`} />
                          </div>
                          <div>
                            <p className="text-sm font-medium text-gray-600">{metric.title}</p>
                            <p className="text-xl font-bold">{metric.value}</p>
                          </div>
                        </div>
                        <div className="text-right">
                          <p className={`text-sm font-medium ${
                            metric.trend === 'up' ? 'text-green-600' : 'text-red-600'
                          }`}>
                            {metric.change}
                          </p>
                          <p className="text-xs text-gray-500">vs prev</p>
                        </div>
                      </div>
                    </CardContent>
                  </Card>
                ))}
              </div>

              {/* Recent Filings */}
              <Card>
                <CardHeader>
                  <CardTitle className="text-lg">Recent Filings</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="space-y-3">
                    {statements.slice(0, 3).map((statement) => (
                      <div
                        key={statement.id}
                        className="flex items-center justify-between p-3 border rounded-lg"
                        onClick={() => setSelectedStatement(statement.id)}
                      >
                        <div className="flex items-center space-x-3">
                          <div className="p-2 bg-blue-100 rounded">
                            <FileText className="h-4 w-4 text-blue-600" />
                          </div>
                          <div>
                            <p className="font-medium">{statement.formType}</p>
                            <p className="text-sm text-gray-500">
                              FY {statement.fiscalYear} Q{statement.fiscalQuarter}
                            </p>
                          </div>
                        </div>
                        <div className="flex items-center space-x-2">
                          {statement.xbrlProcessingStatus === 'completed' ? (
                            <CheckCircle className="h-4 w-4 text-green-600" />
                          ) : (
                            <Clock className="h-4 w-4 text-yellow-600" />
                          )}
                          <ChevronRight className="h-4 w-4 text-gray-400" />
                        </div>
                      </div>
                    ))}
                  </div>
                </CardContent>
              </Card>
            </>
          )}

          {/* Statements Tab */}
          {activeTab === 'statements' && selectedStatement && (
            <div className="space-y-4">
              <Card>
                <CardHeader>
                  <CardTitle className="text-lg">Financial Statement</CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-sm text-gray-600 mb-4">
                    Select a statement to view detailed information.
                  </p>
                  <div className="space-y-2">
                    {statements.map((statement) => (
                      <Button
                        key={statement.id}
                        variant={selectedStatement === statement.id ? 'default' : 'outline'}
                        className="w-full justify-start"
                        onClick={() => setSelectedStatement(statement.id)}
                      >
                        <FileText className="h-4 w-4 mr-2" />
                        {statement.formType} - {statement.periodEndDate}
                      </Button>
                    ))}
                  </div>
                </CardContent>
              </Card>
              
              {/* Statement Details in Drawer */}
              <Drawer>
                <DrawerTrigger asChild>
                  <Button className="w-full">
                    View Statement Details
                  </Button>
                </DrawerTrigger>
                <DrawerContent className="h-[80vh]">
                  <DrawerHeader>
                    <DrawerTitle>Financial Statement Details</DrawerTitle>
                  </DrawerHeader>
                  <div className="flex-1 overflow-y-auto p-4">
                    <FinancialStatementViewer
                      statementId={selectedStatement}
                      companyId={company.id}
                      userType={userType}
                      showEducationalContent={showEducationalContent}
                      showCollaborativeFeatures={showCollaborativeFeatures}
                    />
                  </div>
                </DrawerContent>
              </Drawer>
            </div>
          )}

          {/* Ratios Tab */}
          {activeTab === 'ratios' && (
            <RatioAnalysisPanel
              ratios={ratios}
              loading={false}
              userType={userType}
              showEducationalContent={showEducationalContent}
            />
          )}

          {/* Trends Tab */}
          {activeTab === 'trends' && (
            <TrendAnalysisChart
              ratios={ratios}
              statements={statements}
              timeRange="3Y"
              onTimeRangeChange={() => {}}
            />
          )}

          {/* Alerts Tab */}
          {activeTab === 'alerts' && (
            <FinancialAlerts
              companyId={company.id}
              ratios={ratios}
              statements={statements}
              userType={userType}
            />
          )}
        </div>

        {/* Mobile Bottom Navigation */}
        <div className="fixed bottom-0 left-0 right-0 bg-white border-t shadow-lg">
          <div className="flex">
            {mobileTabs.map((tab) => (
              <button
                key={tab.id}
                className={`flex-1 flex flex-col items-center py-2 ${
                  activeTab === tab.id ? 'text-blue-600' : 'text-gray-500'
                }`}
                onClick={() => setActiveTab(tab.id)}
              >
                <tab.icon className="h-5 w-5 mb-1" />
                <span className="text-xs">{tab.label}</span>
              </button>
            ))}
          </div>
        </div>
      </div>
    );
  }

  // Tablet layout
  if (deviceType === 'tablet') {
    return (
      <div className="min-h-screen bg-gray-50">
        <div className="p-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center space-x-2">
                <Tablet className="h-5 w-5" />
                <span>Tablet View - {company.name}</span>
              </CardTitle>
            </CardHeader>
            <CardContent>
              <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-4">
                <TabsList className="grid w-full grid-cols-5">
                  <TabsTrigger value="overview">Overview</TabsTrigger>
                  <TabsTrigger value="statements">Statements</TabsTrigger>
                  <TabsTrigger value="ratios">Ratios</TabsTrigger>
                  <TabsTrigger value="trends">Trends</TabsTrigger>
                  <TabsTrigger value="alerts">Alerts</TabsTrigger>
                </TabsList>

                <TabsContent value="overview">
                  <div className="grid grid-cols-2 gap-4">
                    {keyMetrics.map((metric, index) => (
                      <Card key={index}>
                        <CardContent className="p-4">
                          <div className="flex items-center justify-between">
                            <div>
                              <p className="text-sm font-medium text-gray-600">{metric.title}</p>
                              <p className="text-xl font-bold">{metric.value}</p>
                            </div>
                            <div className={`p-2 rounded-lg ${
                              metric.trend === 'up' ? 'bg-green-100' : 'bg-red-100'
                            }`}>
                              <metric.icon className={`h-5 w-5 ${
                                metric.trend === 'up' ? 'text-green-600' : 'text-red-600'
                              }`} />
                            </div>
                          </div>
                        </CardContent>
                      </Card>
                    ))}
                  </div>
                </TabsContent>

                <TabsContent value="statements">
                  {selectedStatement ? (
                    <FinancialStatementViewer
                      statementId={selectedStatement}
                      companyId={company.id}
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

                <TabsContent value="ratios">
                  <RatioAnalysisPanel
                    ratios={ratios}
                    loading={false}
                    userType={userType}
                    showEducationalContent={showEducationalContent}
                  />
                </TabsContent>

                <TabsContent value="trends">
                  <TrendAnalysisChart
                    ratios={ratios}
                    statements={statements}
                    timeRange="3Y"
                    onTimeRangeChange={() => {}}
                  />
                </TabsContent>

                <TabsContent value="alerts">
                  <FinancialAlerts
                    companyId={company.id}
                    ratios={ratios}
                    statements={statements}
                    userType={userType}
                  />
                </TabsContent>
              </Tabs>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  // Desktop layout - use full dashboard
  return (
    <div className="min-h-screen bg-gray-50">
      <div className="p-4">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center space-x-2">
              <Monitor className="h-5 w-5" />
              <span>Desktop View - {company.name}</span>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <FinancialDashboard
              companyId={company.id}
              userType={userType}
              showEducationalContent={showEducationalContent}
              showCollaborativeFeatures={showCollaborativeFeatures}
            />
          </CardContent>
        </Card>
      </div>
    </div>
  );
};
