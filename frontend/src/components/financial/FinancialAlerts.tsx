import React, { useState, useEffect, useMemo } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Switch } from '@/components/ui/switch';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import {
  AlertTriangle,
  Bell,
  BellOff,
  TrendingUp,
  TrendingDown,
  DollarSign,
  Calendar,
  Settings,
  Filter,
  Search,
  CheckCircle,
  XCircle,
  Clock,
  Star,
  Eye,
  EyeOff,
} from 'lucide-react';
import { FinancialRatio, FinancialStatement } from '@/types/financial';

interface FinancialAlert {
  id: string;
  type: 'ratio_threshold' | 'trend_change' | 'filing_deadline' | 'benchmark_change' | 'custom';
  severity: 'low' | 'medium' | 'high' | 'critical';
  title: string;
  description: string;
  companyId: string;
  companyName: string;
  ratioName?: string;
  currentValue?: number;
  thresholdValue?: number;
  direction: 'above' | 'below' | 'change';
  isActive: boolean;
  isRead: boolean;
  createdAt: string;
  triggeredAt?: string;
  expiresAt?: string;
  metadata?: Record<string, any>;
}

interface FinancialAlertsProps {
  companyId: string;
  ratios: FinancialRatio[];
  statements: FinancialStatement[];
  userType?: 'beginner' | 'intermediate' | 'advanced' | 'expert';
  onAlertClick?: (alert: FinancialAlert) => void;
}

export const FinancialAlerts: React.FC<FinancialAlertsProps> = ({
  companyId,
  ratios,
  statements,
  userType = 'intermediate',
  onAlertClick,
}) => {
  const [alerts, setAlerts] = useState<FinancialAlert[]>([]);
  const [filterType, setFilterType] = useState<string>('all');
  const [filterSeverity, setFilterSeverity] = useState<string>('all');
  const [showRead, setShowRead] = useState(false);
  const [sortBy, setSortBy] = useState<'date' | 'severity' | 'type'>('date');

  // Mock alerts data - in real implementation, this would come from API
  useEffect(() => {
    const mockAlerts: FinancialAlert[] = [
      {
        id: '1',
        type: 'ratio_threshold',
        severity: 'high',
        title: 'Current Ratio Below Industry Average',
        description: 'Current ratio of 1.04 is below the industry average of 1.5, indicating potential liquidity concerns.',
        companyId,
        companyName: 'Apple Inc.',
        ratioName: 'currentRatio',
        currentValue: 1.04,
        thresholdValue: 1.5,
        direction: 'below',
        isActive: true,
        isRead: false,
        createdAt: '2024-01-15T10:30:00Z',
        triggeredAt: '2024-01-15T10:30:00Z',
      },
      {
        id: '2',
        type: 'trend_change',
        severity: 'medium',
        title: 'ROE Trend Reversal Detected',
        description: 'Return on Equity has declined for 3 consecutive quarters, from 16.2% to 14.7%.',
        companyId,
        companyName: 'Apple Inc.',
        ratioName: 'returnOnEquity',
        currentValue: 0.147,
        direction: 'change',
        isActive: true,
        isRead: false,
        createdAt: '2024-01-14T15:45:00Z',
        triggeredAt: '2024-01-14T15:45:00Z',
      },
      {
        id: '3',
        type: 'benchmark_change',
        severity: 'low',
        title: 'Industry Ranking Improved',
        description: 'Company moved from 45th to 60th percentile in debt-to-equity ratio compared to industry peers.',
        companyId,
        companyName: 'Apple Inc.',
        ratioName: 'debtToEquity',
        currentValue: 1.73,
        direction: 'change',
        isActive: true,
        isRead: true,
        createdAt: '2024-01-13T09:20:00Z',
        triggeredAt: '2024-01-13T09:20:00Z',
      },
      {
        id: '4',
        type: 'filing_deadline',
        severity: 'medium',
        title: '10-Q Filing Due Soon',
        description: 'Quarterly report (10-Q) is due within 5 business days. Ensure all financial data is ready.',
        companyId,
        companyName: 'Apple Inc.',
        direction: 'change',
        isActive: true,
        isRead: false,
        createdAt: '2024-01-12T14:00:00Z',
        expiresAt: '2024-01-25T23:59:59Z',
      },
      {
        id: '5',
        type: 'custom',
        severity: 'critical',
        title: 'Cash Flow Warning',
        description: 'Operating cash flow has decreased by 15% compared to the previous quarter.',
        companyId,
        companyName: 'Apple Inc.',
        direction: 'change',
        isActive: true,
        isRead: false,
        createdAt: '2024-01-11T11:15:00Z',
        triggeredAt: '2024-01-11T11:15:00Z',
      },
    ];

    setAlerts(mockAlerts);
  }, [companyId]);

  // Filter and sort alerts
  const filteredAlerts = useMemo(() => {
    let filtered = alerts.filter(alert => {
      if (!showRead && alert.isRead) return false;
      if (filterType !== 'all' && alert.type !== filterType) return false;
      if (filterSeverity !== 'all' && alert.severity !== filterSeverity) return false;
      return true;
    });

    filtered.sort((a, b) => {
      switch (sortBy) {
        case 'severity':
          const severityOrder = { critical: 4, high: 3, medium: 2, low: 1 };
          return severityOrder[b.severity] - severityOrder[a.severity];
        case 'type':
          return a.type.localeCompare(b.type);
        case 'date':
        default:
          return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
      }
    });

    return filtered;
  }, [alerts, filterType, filterSeverity, showRead, sortBy]);

  // Get severity color and icon
  const getSeverityConfig = (severity: string) => {
    switch (severity) {
      case 'critical':
        return {
          color: 'text-red-600',
          bgColor: 'bg-red-100',
          icon: <AlertTriangle className="h-4 w-4" />,
          badge: 'bg-red-100 text-red-800',
        };
      case 'high':
        return {
          color: 'text-orange-600',
          bgColor: 'bg-orange-100',
          icon: <TrendingDown className="h-4 w-4" />,
          badge: 'bg-orange-100 text-orange-800',
        };
      case 'medium':
        return {
          color: 'text-yellow-600',
          bgColor: 'bg-yellow-100',
          icon: <Clock className="h-4 w-4" />,
          badge: 'bg-yellow-100 text-yellow-800',
        };
      case 'low':
        return {
          color: 'text-blue-600',
          bgColor: 'bg-blue-100',
          icon: <TrendingUp className="h-4 w-4" />,
          badge: 'bg-blue-100 text-blue-800',
        };
      default:
        return {
          color: 'text-gray-600',
          bgColor: 'bg-gray-100',
          icon: <Bell className="h-4 w-4" />,
          badge: 'bg-gray-100 text-gray-800',
        };
    }
  };

  // Get alert type icon
  const getAlertTypeIcon = (type: string) => {
    switch (type) {
      case 'ratio_threshold':
        return <DollarSign className="h-4 w-4" />;
      case 'trend_change':
        return <TrendingDown className="h-4 w-4" />;
      case 'filing_deadline':
        return <Calendar className="h-4 w-4" />;
      case 'benchmark_change':
        return <TrendingUp className="h-4 w-4" />;
      default:
        return <Bell className="h-4 w-4" />;
    }
  };

  // Format alert value
  const formatAlertValue = (value: number, ratioName?: string) => {
    if (!value) return '';
    
    const percentageRatios = ['returnOnEquity', 'returnOnAssets', 'grossMargin', 'netMargin'];
    if (ratioName && percentageRatios.includes(ratioName)) {
      return `${(value * 100).toFixed(1)}%`;
    }
    return value.toFixed(2);
  };

  // Handle alert actions
  const handleMarkAsRead = (alertId: string) => {
    setAlerts(prev => prev.map(alert => 
      alert.id === alertId ? { ...alert, isRead: true } : alert
    ));
  };

  const handleToggleActive = (alertId: string) => {
    setAlerts(prev => prev.map(alert => 
      alert.id === alertId ? { ...alert, isActive: !alert.isActive } : alert
    ));
  };

  const handleAlertClick = (alert: FinancialAlert) => {
    if (!alert.isRead) {
      handleMarkAsRead(alert.id);
    }
    onAlertClick?.(alert);
  };

  // Statistics
  const stats = useMemo(() => {
    const total = alerts.length;
    const unread = alerts.filter(a => !a.isRead).length;
    const critical = alerts.filter(a => a.severity === 'critical' && !a.isRead).length;
    const active = alerts.filter(a => a.isActive).length;

    return { total, unread, critical, active };
  }, [alerts]);

  return (
    <div className="space-y-6">
      {/* Header with Statistics */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center space-x-2">
              <Bell className="h-5 w-5" />
              <span>Financial Alerts</span>
            </CardTitle>
            <div className="flex items-center space-x-4">
              <div className="text-sm text-muted-foreground">
                {stats.unread} unread • {stats.critical} critical • {stats.active} active
              </div>
              <Button variant="outline" size="sm">
                <Settings className="h-4 w-4 mr-2" />
                Settings
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="text-center p-3 bg-blue-50 rounded-lg">
              <div className="text-2xl font-bold text-blue-600">{stats.total}</div>
              <div className="text-sm text-muted-foreground">Total Alerts</div>
            </div>
            <div className="text-center p-3 bg-orange-50 rounded-lg">
              <div className="text-2xl font-bold text-orange-600">{stats.unread}</div>
              <div className="text-sm text-muted-foreground">Unread</div>
            </div>
            <div className="text-center p-3 bg-red-50 rounded-lg">
              <div className="text-2xl font-bold text-red-600">{stats.critical}</div>
              <div className="text-sm text-muted-foreground">Critical</div>
            </div>
            <div className="text-center p-3 bg-green-50 rounded-lg">
              <div className="text-2xl font-bold text-green-600">{stats.active}</div>
              <div className="text-sm text-muted-foreground">Active</div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Filters and Controls */}
      <Card>
        <CardContent className="p-4">
          <div className="flex items-center space-x-4">
            <div className="flex items-center space-x-2">
              <Filter className="h-4 w-4" />
              <span className="text-sm font-medium">Type:</span>
              <Select value={filterType} onValueChange={setFilterType}>
                <SelectTrigger className="w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All</SelectItem>
                  <SelectItem value="ratio_threshold">Ratios</SelectItem>
                  <SelectItem value="trend_change">Trends</SelectItem>
                  <SelectItem value="filing_deadline">Deadlines</SelectItem>
                  <SelectItem value="benchmark_change">Benchmarks</SelectItem>
                  <SelectItem value="custom">Custom</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center space-x-2">
              <span className="text-sm font-medium">Severity:</span>
              <Select value={filterSeverity} onValueChange={setFilterSeverity}>
                <SelectTrigger className="w-32">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All</SelectItem>
                  <SelectItem value="critical">Critical</SelectItem>
                  <SelectItem value="high">High</SelectItem>
                  <SelectItem value="medium">Medium</SelectItem>
                  <SelectItem value="low">Low</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center space-x-2">
              <span className="text-sm font-medium">Sort:</span>
              <Select value={sortBy} onValueChange={(value: string) => setSortBy(value as 'date' | 'severity' | 'type')}>
                <SelectTrigger className="w-24">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="date">Date</SelectItem>
                  <SelectItem value="severity">Severity</SelectItem>
                  <SelectItem value="type">Type</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center space-x-2">
              <Switch
                checked={showRead}
                onCheckedChange={setShowRead}
                id="show-read"
              />
              <label htmlFor="show-read" className="text-sm">
                Show read alerts
              </label>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Alerts List */}
      <div className="space-y-3">
        {filteredAlerts.length === 0 ? (
          <Card>
            <CardContent className="p-8 text-center">
              <BellOff className="h-12 w-12 text-gray-400 mx-auto mb-4" />
              <h3 className="text-lg font-medium text-gray-600 mb-2">No alerts found</h3>
              <p className="text-gray-500">
                {showRead ? 'No alerts match your current filters.' : 'All caught up! No unread alerts.'}
              </p>
            </CardContent>
          </Card>
        ) : (
          filteredAlerts.map((alert) => {
            const severityConfig = getSeverityConfig(alert.severity);
            const isExpiring = alert.expiresAt && new Date(alert.expiresAt) < new Date(Date.now() + 7 * 24 * 60 * 60 * 1000);

            return (
              <Card 
                key={alert.id} 
                className={`cursor-pointer hover:shadow-md transition-shadow ${
                  !alert.isRead ? 'ring-2 ring-blue-200' : ''
                } ${!alert.isActive ? 'opacity-60' : ''}`}
                onClick={() => handleAlertClick(alert)}
              >
                <CardContent className="p-4">
                  <div className="flex items-start space-x-3">
                    <div className={`p-2 rounded-lg ${severityConfig.bgColor}`}>
                      {severityConfig.icon}
                    </div>
                    
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center justify-between mb-2">
                        <div className="flex items-center space-x-2">
                          <h3 className="font-medium text-gray-900 truncate">
                            {alert.title}
                          </h3>
                          {!alert.isRead && (
                            <div className="w-2 h-2 bg-blue-500 rounded-full" />
                          )}
                        </div>
                        <div className="flex items-center space-x-2">
                          <Badge className={severityConfig.badge}>
                            {alert.severity}
                          </Badge>
                          <Badge variant="outline">
                            {alert.type.replace('_', ' ')}
                          </Badge>
                        </div>
                      </div>

                      <p className="text-sm text-gray-600 mb-3">
                        {alert.description}
                      </p>

                      <div className="flex items-center justify-between">
                        <div className="flex items-center space-x-4 text-sm text-gray-500">
                          <div className="flex items-center space-x-1">
                            {getAlertTypeIcon(alert.type)}
                            <span>{alert.companyName}</span>
                          </div>
                          
                          {alert.ratioName && alert.currentValue && (
                            <div className="flex items-center space-x-1">
                              <span>Current:</span>
                              <span className="font-medium">
                                {formatAlertValue(alert.currentValue, alert.ratioName)}
                              </span>
                            </div>
                          )}

                          {alert.thresholdValue && (
                            <div className="flex items-center space-x-1">
                              <span>Threshold:</span>
                              <span className="font-medium">
                                {formatAlertValue(alert.thresholdValue, alert.ratioName)}
                              </span>
                            </div>
                          )}

                          <span>
                            {new Date(alert.createdAt).toLocaleDateString()}
                          </span>
                        </div>

                        <div className="flex items-center space-x-2">
                          {isExpiring && (
                            <Badge variant="destructive" className="text-xs">
                              Expires Soon
                            </Badge>
                          )}
                          
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleToggleActive(alert.id);
                            }}
                          >
                            {alert.isActive ? (
                              <Eye className="h-4 w-4" />
                            ) : (
                              <EyeOff className="h-4 w-4" />
                            )}
                          </Button>

                          {!alert.isRead && (
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={(e) => {
                                e.stopPropagation();
                                handleMarkAsRead(alert.id);
                              }}
                            >
                              <CheckCircle className="h-4 w-4" />
                            </Button>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>
                </CardContent>
              </Card>
            );
          })
        )}
      </div>
    </div>
  );
};
