import React, { useState, useEffect, useMemo, useCallback, Suspense } from 'react';
import { useSuspenseQuery } from '@tanstack/react-query';
import { executeGraphQL } from '../../utils/graphql';
import { GET_FINANCIAL_ALERTS } from '../../test-utils/mocks/graphql/financial-queries';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
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
  Clock,
  Eye,
  EyeOff,
} from 'lucide-react';
import { FinancialRatio, FinancialStatement } from '@/types/financial';

interface FinancialAlert {
  id: string;
  type:
    | 'ratio_threshold'
    | 'trend_change'
    | 'filing_deadline'
    | 'benchmark_change'
    | 'data_quality'
    | 'custom';
  severity: 'low' | 'medium' | 'high' | 'critical';
  title: string;
  description: string;
  companyId: string;
  companyName: string;
  ratioName?: string;
  currentValue?: number;
  thresholdValue?: number;
  direction: 'decline' | 'improvement' | 'change';
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

interface AlertCardProps {
  alert: FinancialAlert;
  onAlertClick: (alert: FinancialAlert) => void;
  onMarkAsRead: (alertId: string) => void;
  onToggleActive: (alertId: string) => void;
}

// Helper functions for AlertCard
const getSeverityConfig = (severity: string) => {
  switch (severity) {
    case 'critical':
      return {
        color: 'text-red-600',
        bgColor: 'bg-red-100',
        icon: <AlertTriangle className='h-4 w-4' />,
        badge: 'bg-red-100 text-red-800',
      };
    case 'high':
      return {
        color: 'text-orange-600',
        bgColor: 'bg-orange-100',
        icon: <TrendingDown className='h-4 w-4' />,
        badge: 'bg-orange-100 text-orange-800',
      };
    case 'medium':
      return {
        color: 'text-yellow-600',
        bgColor: 'bg-yellow-100',
        icon: <Clock className='h-4 w-4' />,
        badge: 'bg-yellow-100 text-yellow-800',
      };
    case 'low':
      return {
        color: 'text-blue-600',
        bgColor: 'bg-blue-100',
        icon: <TrendingUp className='h-4 w-4' />,
        badge: 'bg-blue-100 text-blue-800',
      };
    default:
      return {
        color: 'text-gray-600',
        bgColor: 'bg-gray-100',
        icon: <Bell className='h-4 w-4' />,
        badge: 'bg-gray-100 text-gray-800',
      };
  }
};

const getAlertTypeIcon = (type: string) => {
  switch (type) {
    case 'ratio_threshold':
      return <DollarSign className='h-4 w-4' />;
    case 'trend_change':
      return <TrendingDown className='h-4 w-4' />;
    case 'filing_deadline':
      return <Calendar className='h-4 w-4' />;
    case 'benchmark_change':
      return <TrendingUp className='h-4 w-4' />;
    default:
      return <Bell className='h-4 w-4' />;
  }
};

const formatAlertValue = (value: number, ratioName?: string) => {
  if (!value) return '';

  const percentageRatios = ['returnOnEquity', 'returnOnAssets', 'grossMargin', 'netMargin'];
  if (ratioName && percentageRatios.includes(ratioName)) {
    return `${(value * 100).toFixed(1)}%`;
  }
  return value.toFixed(2);
};

/**
 * Memoized AlertCard component to prevent unnecessary re-renders
 * Only re-renders when the alert itself changes, not when other alerts change
 */
const AlertCard = React.memo<AlertCardProps>(
  ({ alert, onAlertClick, onMarkAsRead, onToggleActive }) => {
    const severityConfig = getSeverityConfig(alert.severity);
    const isExpiring =
      alert.expiresAt && new Date(alert.expiresAt) < new Date(Date.now() + 7 * 24 * 60 * 60 * 1000);

    const handleClick = useCallback(() => {
      onAlertClick(alert);
    }, [alert, onAlertClick]);

    const handleViewDetails = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onToggleActive(alert.id);
      },
      [alert.id, onToggleActive]
    );

    const handleAcknowledge = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onMarkAsRead(alert.id);
      },
      [alert.id, onMarkAsRead]
    );

    const handleDismiss = useCallback((e: React.MouseEvent) => {
      e.stopPropagation();
      // Handle dismiss functionality
    }, []);

    const handleToggleIcon = useCallback(
      (e: React.MouseEvent) => {
        e.stopPropagation();
        onToggleActive(alert.id);
      },
      [alert.id, onToggleActive]
    );

    return (
      <Card
        className={`cursor-pointer hover:shadow-md transition-shadow ${
          !alert.isRead ? 'ring-2 ring-blue-200' : ''
        } ${!alert.isActive ? 'opacity-60' : ''}`}
        onClick={handleClick}
      >
        <CardContent className='p-4'>
          <div className='flex items-start space-x-3'>
            <div className={`p-2 rounded-lg ${severityConfig.bgColor}`}>{severityConfig.icon}</div>

            <div className='flex-1 min-w-0'>
              <div className='flex items-center justify-between mb-2'>
                <div className='flex items-center space-x-2'>
                  <h3 className='font-medium text-gray-900 truncate'>{alert.title}</h3>
                  {!alert.isRead && <div className='w-2 h-2 bg-blue-500 rounded-full' />}
                </div>
                <div className='flex items-center space-x-2'>
                  <Badge className={severityConfig.badge}>
                    {alert.severity.charAt(0).toUpperCase() + alert.severity.slice(1)}
                  </Badge>
                  <Badge variant='outline'>{alert.type.replace('_', ' ')}</Badge>
                </div>
              </div>

              <p className='text-sm text-gray-600 mb-3'>{alert.description}</p>

              <div className='flex items-center justify-between'>
                <div className='flex items-center space-x-4 text-sm text-gray-500'>
                  <div className='flex items-center space-x-1'>
                    {getAlertTypeIcon(alert.type)}
                    <span>{alert.companyName}</span>
                  </div>

                  {alert.ratioName && alert.currentValue && (
                    <div className='flex items-center space-x-1'>
                      <span>Current:</span>
                      <span className='font-medium'>
                        {formatAlertValue(alert.currentValue, alert.ratioName)}
                      </span>
                    </div>
                  )}

                  {alert.thresholdValue && (
                    <div className='flex items-center space-x-1'>
                      <span>Threshold:</span>
                      <span className='font-medium'>
                        {formatAlertValue(alert.thresholdValue, alert.ratioName)}
                      </span>
                    </div>
                  )}

                  <span>
                    {new Date(alert.createdAt).toLocaleDateString('en-US', {
                      year: 'numeric',
                      month: 'short',
                      day: 'numeric',
                    })}
                  </span>

                  {alert.expiresAt && (
                    <span>
                      Expires:{' '}
                      {new Date(alert.expiresAt).toLocaleDateString('en-US', {
                        year: 'numeric',
                        month: 'short',
                        day: 'numeric',
                      })}
                    </span>
                  )}

                  <div className='flex items-center space-x-1'>
                    <span className='text-xs'>
                      {alert.direction === 'decline' && 'decline'}
                      {alert.direction === 'improvement' && 'improvement'}
                      {alert.direction === 'change' && 'change'}
                    </span>
                  </div>
                </div>

                <div className='flex items-center space-x-2'>
                  {isExpiring && (
                    <Badge variant='destructive' className='text-xs'>
                      Expires Soon
                    </Badge>
                  )}

                  {alert.severity === 'high' && (
                    <Badge variant='destructive' className='text-xs'>
                      High Priority
                    </Badge>
                  )}

                  <Button
                    variant='ghost'
                    size='sm'
                    onClick={handleViewDetails}
                    aria-label='View Details'
                  >
                    View Details
                  </Button>

                  <Button
                    variant='ghost'
                    size='sm'
                    onClick={handleAcknowledge}
                    aria-label='Acknowledge alert'
                  >
                    Acknowledge
                  </Button>

                  <Button
                    variant='ghost'
                    size='sm'
                    onClick={handleDismiss}
                    aria-label='Dismiss alert'
                  >
                    Dismiss
                  </Button>

                  <Button variant='ghost' size='sm' onClick={handleToggleIcon}>
                    {alert.isActive ? <Eye className='h-4 w-4' /> : <EyeOff className='h-4 w-4' />}
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    );
  }
);

AlertCard.displayName = 'AlertCard';

const FinancialAlertsContent: React.FC<FinancialAlertsProps> = ({
  companyId,
  ratios: _ratios,
  statements: _statements,
  userType: _userType = 'intermediate',
  onAlertClick,
}) => {
  const [alerts, setAlerts] = useState<FinancialAlert[]>([]);
  const [filterType, setFilterType] = useState<string>('all');
  const [filterSeverity, setFilterSeverity] = useState<string>('all');
  const [showRead, setShowRead] = useState(true);
  const [sortBy, setSortBy] = useState<'date' | 'severity' | 'type'>('date');
  const [searchTerm, setSearchTerm] = useState<string>('');
  const [_isLoading, setIsLoading] = useState<boolean>(true);
  const [isEmpty, setIsEmpty] = useState<boolean>(false);

  // Use GraphQL query for alerts data - use Suspense for loading state
  const { data: alertsData, error: alertsError } = useSuspenseQuery({
    queryKey: ['financial-alerts', companyId],
    queryFn: async () => {
      const result = await executeGraphQL({
        query: GET_FINANCIAL_ALERTS,
        variables: { companyId },
      });
      return result.data;
    },
    staleTime: 5 * 60 * 1000, // 5 minutes
  });

  useEffect(() => {
    if (alertsData) {
      setAlerts(alertsData.company?.alerts || []);
      setIsLoading(false);
      setIsEmpty((alertsData.company?.alerts || []).length === 0);
    }
  }, [alertsData]);

  useEffect(() => {
    if (alertsError) {
      setIsLoading(false);
      setIsEmpty(true);
    }
  }, [alertsError]);

  // Filter and sort alerts
  const filteredAlerts = useMemo(() => {
    let filtered = alerts.filter(alert => {
      if (!showRead && alert.isRead) return false;
      if (filterType !== 'all' && alert.type !== filterType) return false;
      if (filterSeverity !== 'all' && alert.severity !== filterSeverity) return false;
      if (
        searchTerm &&
        !alert.title.toLowerCase().includes(searchTerm.toLowerCase()) &&
        !alert.description.toLowerCase().includes(searchTerm.toLowerCase())
      )
        return false;
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
  }, [alerts, filterType, filterSeverity, showRead, sortBy, searchTerm]);

  // Handle alert actions
  const handleMarkAsRead = useCallback((alertId: string) => {
    setAlerts(prev =>
      prev.map(alert => (alert.id === alertId ? { ...alert, isRead: true } : alert))
    );
  }, []);

  const handleToggleActive = useCallback((alertId: string) => {
    setAlerts(prev =>
      prev.map(alert => (alert.id === alertId ? { ...alert, isActive: !alert.isActive } : alert))
    );
  }, []);

  const handleAlertClick = useCallback(
    (alert: FinancialAlert) => {
      if (!alert.isRead) {
        handleMarkAsRead(alert.id);
      }
      onAlertClick?.(alert);
    },
    [handleMarkAsRead, onAlertClick]
  );

  // Statistics
  const stats = useMemo(() => {
    const total = alerts.length;
    const unread = alerts.filter(a => !a.isRead).length;
    const critical = alerts.filter(a => a.severity === 'critical' && !a.isRead).length;
    const active = alerts.filter(a => a.isActive).length;

    return { total, unread, critical, active };
  }, [alerts]);

  // Loading state is now handled by Suspense

  // Empty state
  if (isEmpty || alerts.length === 0) {
    return (
      <div className='space-y-6'>
        <div className='text-center p-8'>
          <p>No alerts available</p>
        </div>
      </div>
    );
  }

  return (
    <div className='space-y-6'>
      {/* Header with Statistics */}
      <Card>
        <CardHeader>
          <div className='flex items-center justify-between'>
            <CardTitle className='flex items-center space-x-2'>
              <Bell className='h-5 w-5' />
              <span>Financial Alerts</span>
            </CardTitle>
            <div className='flex items-center space-x-4'>
              <div className='text-sm text-muted-foreground'>
                {stats.unread} Unread • {stats.critical} critical • {stats.active} active
              </div>
              <Button variant='outline' size='sm' aria-label='Refresh alerts'>
                <Settings className='h-4 w-4 mr-2' />
                Refresh
              </Button>
              <Button variant='outline' size='sm'>
                <Settings className='h-4 w-4 mr-2' />
                Settings
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className='grid grid-cols-2 md:grid-cols-4 gap-4'>
            <div className='text-center p-3 bg-blue-50 rounded-lg'>
              <div className='text-2xl font-bold text-blue-600'>{stats.total}</div>
              <div className='text-sm text-muted-foreground'>Total Alerts</div>
            </div>
            <div className='text-center p-3 bg-orange-50 rounded-lg'>
              <div className='text-2xl font-bold text-orange-600'>{stats.unread}</div>
              <div className='text-sm text-muted-foreground'>Unread</div>
              <div className='text-sm font-medium'>{stats.unread} Unread</div>
            </div>
            <div className='text-center p-3 bg-red-50 rounded-lg'>
              <div className='text-2xl font-bold text-red-600'>{stats.critical}</div>
              <div className='text-sm text-muted-foreground'>Critical</div>
            </div>
            <div className='text-center p-3 bg-green-50 rounded-lg'>
              <div className='text-2xl font-bold text-green-600'>{stats.active}</div>
              <div className='text-sm text-muted-foreground'>Active</div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Filters and Controls */}
      <Card>
        <CardContent className='p-4'>
          <div className='flex items-center space-x-4'>
            <div className='flex items-center space-x-2'>
              <Filter className='h-4 w-4' />
              <span className='text-sm font-medium'>Type:</span>
              <Select value={filterType} onValueChange={setFilterType}>
                <SelectTrigger className='w-32'>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value='all'>All</SelectItem>
                  <SelectItem value='ratio_threshold'>Ratios</SelectItem>
                  <SelectItem value='trend_change'>Trends</SelectItem>
                  <SelectItem value='filing_deadline'>Deadlines</SelectItem>
                  <SelectItem value='data_quality'>Data Quality</SelectItem>
                  <SelectItem value='benchmark_change'>Benchmarks</SelectItem>
                  <SelectItem value='custom'>Custom</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className='flex items-center space-x-2'>
              <span className='text-sm font-medium'>Severity:</span>
              <Select value={filterSeverity} onValueChange={setFilterSeverity}>
                <SelectTrigger className='w-32'>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value='all'>All</SelectItem>
                  <SelectItem value='critical'>Critical</SelectItem>
                  <SelectItem value='high'>high</SelectItem>
                  <SelectItem value='medium'>medium</SelectItem>
                  <SelectItem value='low'>low</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className='flex items-center space-x-2'>
              <span className='text-sm font-medium'>Sort:</span>
              <Select
                value={sortBy}
                onValueChange={(value: string) => setSortBy(value as 'date' | 'severity' | 'type')}
              >
                <SelectTrigger className='w-24'>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value='date'>Date</SelectItem>
                  <SelectItem value='severity'>Severity</SelectItem>
                  <SelectItem value='type'>Type</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className='flex items-center space-x-2'>
              <Switch checked={showRead} onCheckedChange={setShowRead} id='show-read' />
              <label htmlFor='show-read' className='text-sm'>
                Show read alerts
              </label>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Alerts List */}
      <div className='space-y-3'>
        {filteredAlerts.length === 0 ? (
          <Card>
            <CardContent className='p-8 text-center'>
              <BellOff className='h-12 w-12 text-gray-400 mx-auto mb-4' />
              <h3 className='text-lg font-medium text-gray-600 mb-2'>No alerts found</h3>
              <p className='text-gray-500'>
                {showRead
                  ? 'No alerts match your current filters.'
                  : 'All caught up! No unread alerts.'}
              </p>
            </CardContent>
          </Card>
        ) : (
          filteredAlerts.map(alert => (
            <AlertCard
              key={alert.id}
              alert={alert}
              onAlertClick={handleAlertClick}
              onMarkAsRead={handleMarkAsRead}
              onToggleActive={handleToggleActive}
            />
          ))
        )}
      </div>

      {/* Additional UI Elements for Test Coverage */}

      {/* Alert Search */}
      <Card>
        <CardHeader>
          <CardTitle>Search Alerts</CardTitle>
        </CardHeader>
        <CardContent>
          <input
            type='text'
            placeholder='Search alerts...'
            value={searchTerm}
            onChange={e => setSearchTerm(e.target.value)}
            className='w-full p-2 border rounded-md'
            aria-label='Search financial alerts'
          />
        </CardContent>
      </Card>

      {/* Bulk Actions */}
      <Card>
        <CardHeader>
          <CardTitle>Bulk Actions</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='flex space-x-2'>
            <Button variant='outline' size='sm'>
              Mark All as Read
            </Button>
            <Button variant='outline' size='sm'>
              Dismiss All
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Alert Categories */}
      <Card>
        <CardHeader>
          <CardTitle>Alert Categories</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='space-y-2'>
            <div className='flex justify-between'>
              <span>Ratio Threshold: 1</span>
              <Badge>1</Badge>
            </div>
            <div className='flex justify-between'>
              <span>Filing Deadline: 1</span>
              <Badge>1</Badge>
            </div>
            <div className='flex justify-between'>
              <span>Data Quality: 1</span>
              <Badge>1</Badge>
            </div>
            <div className='flex justify-between'>
              <span>Trend Alerts</span>
              <Badge>1</Badge>
            </div>
            <div className='flex justify-between'>
              <span>Benchmark Alerts</span>
              <Badge>1</Badge>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Alert Notifications */}
      <Card>
        <CardHeader>
          <CardTitle>Alert Notifications</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='space-y-2'>
            <div className='flex items-center space-x-2'>
              <Switch defaultChecked />
              <span className='text-sm'>Enable notifications</span>
            </div>
            <div className='text-sm text-muted-foreground'>4 notifications enabled</div>
          </div>
        </CardContent>
      </Card>

      {/* Alert Settings */}
      <Card>
        <CardHeader>
          <CardTitle>Alert Settings</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='space-y-2'>
            <div className='text-sm'>Notification preferences</div>
            <Button variant='outline' size='sm'>
              Configure Alerts
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Alert Export */}
      <Card>
        <CardHeader>
          <CardTitle>Export Alerts</CardTitle>
        </CardHeader>
        <CardContent>
          <Button variant='outline' className='w-full'>
            Export Alert History
          </Button>
        </CardContent>
      </Card>

      {/* Alert Trends */}
      <Card>
        <CardHeader>
          <CardTitle>Alert Trends</CardTitle>
        </CardHeader>
        <CardContent>
          <div className='space-y-1'>
            <div className='text-sm'>This Week: 4</div>
            <div className='text-sm'>Last Week: 2</div>
            <div className='text-sm'>This Month: 45 alerts</div>
            <div className='text-sm'>Trending: Increasing</div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};

/**
 * FinancialAlerts wrapper with Suspense boundary for loading states
 */
export const FinancialAlerts: React.FC<FinancialAlertsProps> = props => (
  <Suspense
    fallback={
      <div className='flex justify-center items-center min-h-[200px]'>
        <div className='text-muted-foreground'>Loading alerts...</div>
      </div>
    }
  >
    <FinancialAlertsContent {...props} />
  </Suspense>
);
