import React, { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { Progress } from '@/components/ui/progress';
import { Alert, AlertDescription } from '@/components/ui/alert';
import {
  Download,
  FileText,
  FileSpreadsheet,
  File,
  Image,
  CheckCircle,
  Clock,
  AlertCircle,
  Share2,
  Mail,
  Printer,
  Database,
} from 'lucide-react';
import { FinancialStatement, FinancialRatio, Company } from '@/types/financial';

interface ExportFormat {
  id: string;
  name: string;
  extension: string;
  icon: React.ReactNode;
  description: string;
  supportsCharts: boolean;
  supportsFilters: boolean;
}

interface ExportOptions {
  format: string;
  includeCharts: boolean;
  includeRawData: boolean;
  includeAnnotations: boolean;
  includeBenchmarks: boolean;
  dateRange: {
    start: string;
    end: string;
  };
  selectedStatements: string[];
  selectedRatios: string[];
  chartTypes: string[];
  compression: 'none' | 'zip' | 'gzip';
  password?: string;
}

interface ExportJob {
  id: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  format: string;
  fileName: string;
  progress: number;
  createdAt: string;
  completedAt?: string;
  downloadUrl?: string;
  errorMessage?: string;
}

interface FinancialExportProps {
  company: Company;
  statements: FinancialStatement[];
  ratios: FinancialRatio[];
  userType?: 'beginner' | 'intermediate' | 'advanced' | 'expert';
}

export const FinancialExport: React.FC<FinancialExportProps> = ({
  company,
  statements,
  ratios,
  userType = 'intermediate',
}) => {
  const [exportOptions, setExportOptions] = useState<ExportOptions>({
    format: 'pdf',
    includeCharts: true,
    includeRawData: false,
    includeAnnotations: true,
    includeBenchmarks: true,
    dateRange: {
      start: '',
      end: '',
    },
    selectedStatements: [],
    selectedRatios: [],
    chartTypes: ['line', 'bar'],
    compression: 'none',
  });

  const [exportJobs, setExportJobs] = useState<ExportJob[]>([]);
  const [isExporting, setIsExporting] = useState(false);

  const exportFormats: ExportFormat[] = [
    {
      id: 'pdf',
      name: 'PDF Report',
      extension: 'pdf',
      icon: <File className='h-5 w-5' />,
      description: 'Professional PDF report with charts and analysis',
      supportsCharts: true,
      supportsFilters: true,
    },
    {
      id: 'excel',
      name: 'Excel Workbook',
      extension: 'xlsx',
      icon: <FileSpreadsheet className='h-5 w-5' />,
      description: 'Excel file with multiple sheets and raw data',
      supportsCharts: true,
      supportsFilters: true,
    },
    {
      id: 'csv',
      name: 'CSV Data',
      extension: 'csv',
      icon: <FileText className='h-5 w-5' />,
      description: 'Raw data in CSV format for analysis',
      supportsCharts: false,
      supportsFilters: false,
    },
    {
      id: 'json',
      name: 'JSON Data',
      extension: 'json',
      icon: <Database className='h-5 w-5' />,
      description: 'Structured JSON data with metadata',
      supportsCharts: false,
      supportsFilters: false,
    },
    {
      id: 'png',
      name: 'Chart Images',
      extension: 'png',
      icon: <Image className='h-5 w-5' />,
      description: 'High-resolution chart images',
      supportsCharts: true,
      supportsFilters: false,
    },
  ];

  const chartTypes = [
    { id: 'line', name: 'Line Charts', description: 'Trend analysis over time' },
    { id: 'bar', name: 'Bar Charts', description: 'Comparison between periods' },
    { id: 'pie', name: 'Pie Charts', description: 'Component breakdown' },
    { id: 'radar', name: 'Radar Charts', description: 'Multi-dimensional comparison' },
    { id: 'scatter', name: 'Scatter Plots', description: 'Correlation analysis' },
  ];

  const handleFormatChange = (format: string) => {
    const selectedFormat = exportFormats.find(f => f.id === format);
    setExportOptions(prev => ({
      ...prev,
      format,
      includeCharts: selectedFormat?.supportsCharts || false,
    }));
  };

  const handleStatementSelection = (statementId: string, checked: boolean) => {
    setExportOptions(prev => ({
      ...prev,
      selectedStatements: checked
        ? [...prev.selectedStatements, statementId]
        : prev.selectedStatements.filter(id => id !== statementId),
    }));
  };

  const handleRatioSelection = (ratioId: string, checked: boolean) => {
    setExportOptions(prev => ({
      ...prev,
      selectedRatios: checked
        ? [...prev.selectedRatios, ratioId]
        : prev.selectedRatios.filter(id => id !== ratioId),
    }));
  };

  const handleChartTypeToggle = (chartType: string, checked: boolean) => {
    setExportOptions(prev => ({
      ...prev,
      chartTypes: checked
        ? [...prev.chartTypes, chartType]
        : prev.chartTypes.filter(type => type !== chartType),
    }));
  };

  const handleSelectAllStatements = (checked: boolean) => {
    setExportOptions(prev => ({
      ...prev,
      selectedStatements: checked ? statements.map(s => s.id) : [],
    }));
  };

  const handleSelectAllRatios = (checked: boolean) => {
    setExportOptions(prev => ({
      ...prev,
      selectedRatios: checked ? ratios.map(r => r.id) : [],
    }));
  };

  const startExport = async () => {
    setIsExporting(true);

    const job: ExportJob = {
      id: `export-${Date.now()}`,
      status: 'processing',
      format: exportOptions.format,
      fileName: `${company.name}-financial-export-${new Date().toISOString().split('T')[0]}.${exportFormats.find(f => f.id === exportOptions.format)?.extension}`,
      progress: 0,
      createdAt: new Date().toISOString(),
    };

    setExportJobs(prev => [job, ...prev]);

    // Simulate export process
    const interval = setInterval(() => {
      setExportJobs(prev =>
        prev.map(j => {
          if (j.id === job.id) {
            const newProgress = Math.min(j.progress + Math.random() * 20, 100);
            if (newProgress >= 100) {
              clearInterval(interval);
              return {
                ...j,
                status: 'completed',
                progress: 100,
                completedAt: new Date().toISOString(),
                downloadUrl: `/downloads/${job.fileName}`,
              };
            }
            return { ...j, progress: newProgress };
          }
          return j;
        })
      );
    }, 500);

    setTimeout(() => {
      setIsExporting(false);
    }, 5000);
  };

  const downloadExport = (job: ExportJob) => {
    if (job.downloadUrl) {
      // In real implementation, this would trigger actual download
      console.log('Downloading:', job.downloadUrl);
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'completed':
        return <CheckCircle className='h-4 w-4 text-green-600' />;
      case 'processing':
        return <Clock className='h-4 w-4 text-blue-600' />;
      case 'failed':
        return <AlertCircle className='h-4 w-4 text-red-600' />;
      default:
        return <Clock className='h-4 w-4 text-gray-600' />;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'completed':
        return 'bg-green-100 text-green-800';
      case 'processing':
        return 'bg-blue-100 text-blue-800';
      case 'failed':
        return 'bg-red-100 text-red-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  };

  return (
    <div className='space-y-6'>
      {/* Header */}
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center space-x-2'>
            <Download className='h-5 w-5' />
            <span>Export Financial Data</span>
          </CardTitle>
          <p className='text-sm text-muted-foreground'>
            Export financial statements, ratios, and analysis in various formats
          </p>
        </CardHeader>
      </Card>

      <div className='grid grid-cols-1 lg:grid-cols-3 gap-6'>
        {/* Export Options */}
        <div className='lg:col-span-2 space-y-6'>
          {/* Format Selection */}
          <Card>
            <CardHeader>
              <CardTitle>Export Format</CardTitle>
            </CardHeader>
            <CardContent>
              <div className='grid grid-cols-1 md:grid-cols-2 gap-3'>
                {exportFormats.map(format => (
                  <div
                    key={format.id}
                    className={`p-4 border rounded-lg cursor-pointer transition-colors ${
                      exportOptions.format === format.id
                        ? 'border-blue-500 bg-blue-50'
                        : 'border-gray-200 hover:border-gray-300'
                    }`}
                    onClick={() => handleFormatChange(format.id)}
                  >
                    <div className='flex items-center space-x-3'>
                      <div className='text-blue-600'>{format.icon}</div>
                      <div className='flex-1'>
                        <h3 className='font-medium'>{format.name}</h3>
                        <p className='text-sm text-muted-foreground'>{format.description}</p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>

          {/* Content Options */}
          <Card>
            <CardHeader>
              <CardTitle>Content Options</CardTitle>
            </CardHeader>
            <CardContent className='space-y-4'>
              <div className='flex items-center space-x-2'>
                <Checkbox
                  id='includeCharts'
                  checked={exportOptions.includeCharts}
                  onCheckedChange={(checked: boolean) =>
                    setExportOptions(prev => ({ ...prev, includeCharts: !!checked }))
                  }
                />
                <label htmlFor='includeCharts' className='text-sm'>
                  Include charts and visualizations
                </label>
              </div>

              <div className='flex items-center space-x-2'>
                <Checkbox
                  id='includeRawData'
                  checked={exportOptions.includeRawData}
                  onCheckedChange={(checked: boolean) =>
                    setExportOptions(prev => ({ ...prev, includeRawData: !!checked }))
                  }
                />
                <label htmlFor='includeRawData' className='text-sm'>
                  Include raw financial data
                </label>
              </div>

              <div className='flex items-center space-x-2'>
                <Checkbox
                  id='includeAnnotations'
                  checked={exportOptions.includeAnnotations}
                  onCheckedChange={(checked: boolean) =>
                    setExportOptions(prev => ({ ...prev, includeAnnotations: !!checked }))
                  }
                />
                <label htmlFor='includeAnnotations' className='text-sm'>
                  Include annotations and comments
                </label>
              </div>

              <div className='flex items-center space-x-2'>
                <Checkbox
                  id='includeBenchmarks'
                  checked={exportOptions.includeBenchmarks}
                  onCheckedChange={(checked: boolean) =>
                    setExportOptions(prev => ({ ...prev, includeBenchmarks: !!checked }))
                  }
                />
                <label htmlFor='includeBenchmarks' className='text-sm'>
                  Include industry benchmarks
                </label>
              </div>
            </CardContent>
          </Card>

          {/* Chart Types (if charts are enabled) */}
          {exportOptions.includeCharts && (
            <Card>
              <CardHeader>
                <CardTitle>Chart Types</CardTitle>
              </CardHeader>
              <CardContent>
                <div className='grid grid-cols-1 md:grid-cols-2 gap-3'>
                  {chartTypes.map(chartType => (
                    <div key={chartType.id} className='flex items-center space-x-2'>
                      <Checkbox
                        id={chartType.id}
                        checked={exportOptions.chartTypes.includes(chartType.id)}
                        onCheckedChange={(checked: boolean) =>
                          handleChartTypeToggle(chartType.id, !!checked)
                        }
                      />
                      <label htmlFor={chartType.id} className='text-sm'>
                        <div className='font-medium'>{chartType.name}</div>
                        <div className='text-muted-foreground'>{chartType.description}</div>
                      </label>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}

          {/* Data Selection */}
          <Card>
            <CardHeader>
              <CardTitle>Data Selection</CardTitle>
            </CardHeader>
            <CardContent className='space-y-4'>
              {/* Statements */}
              <div>
                <div className='flex items-center justify-between mb-3'>
                  <h3 className='font-medium'>Financial Statements</h3>
                  <div className='flex items-center space-x-2'>
                    <Checkbox
                      id='selectAllStatements'
                      checked={exportOptions.selectedStatements.length === statements.length}
                      onCheckedChange={handleSelectAllStatements}
                    />
                    <label htmlFor='selectAllStatements' className='text-sm'>
                      Select All
                    </label>
                  </div>
                </div>
                <div className='space-y-2 max-h-32 overflow-y-auto'>
                  {statements.map(statement => (
                    <div key={statement.id} className='flex items-center space-x-2'>
                      <Checkbox
                        id={`statement-${statement.id}`}
                        checked={exportOptions.selectedStatements.includes(statement.id)}
                        onCheckedChange={(checked: boolean) =>
                          handleStatementSelection(statement.id, !!checked)
                        }
                      />
                      <label htmlFor={`statement-${statement.id}`} className='text-sm'>
                        <div className='font-medium'>{statement.formType}</div>
                        <div className='text-muted-foreground'>
                          FY {statement.fiscalYear} Q{statement.fiscalQuarter} -{' '}
                          {statement.periodEndDate}
                        </div>
                      </label>
                    </div>
                  ))}
                </div>
              </div>

              {/* Ratios */}
              <div>
                <div className='flex items-center justify-between mb-3'>
                  <h3 className='font-medium'>Financial Ratios</h3>
                  <div className='flex items-center space-x-2'>
                    <Checkbox
                      id='selectAllRatios'
                      checked={exportOptions.selectedRatios.length === ratios.length}
                      onCheckedChange={handleSelectAllRatios}
                    />
                    <label htmlFor='selectAllRatios' className='text-sm'>
                      Select All
                    </label>
                  </div>
                </div>
                <div className='space-y-2 max-h-32 overflow-y-auto'>
                  {ratios.map(ratio => (
                    <div key={ratio.id} className='flex items-center space-x-2'>
                      <Checkbox
                        id={`ratio-${ratio.id}`}
                        checked={exportOptions.selectedRatios.includes(ratio.id)}
                        onCheckedChange={(checked: boolean) =>
                          handleRatioSelection(ratio.id, !!checked)
                        }
                      />
                      <label htmlFor={`ratio-${ratio.id}`} className='text-sm'>
                        <div className='font-medium'>{ratio.ratioDisplayName}</div>
                        <div className='text-muted-foreground'>{ratio.category}</div>
                      </label>
                    </div>
                  ))}
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Export Button */}
          <Card>
            <CardContent className='p-6'>
              <Button onClick={startExport} disabled={isExporting} className='w-full' size='lg'>
                <Download className='h-5 w-5 mr-2' />
                {isExporting ? 'Exporting...' : 'Start Export'}
              </Button>
            </CardContent>
          </Card>
        </div>

        {/* Export Jobs */}
        <div className='space-y-6'>
          <Card>
            <CardHeader>
              <CardTitle>Export Jobs</CardTitle>
            </CardHeader>
            <CardContent>
              <div className='space-y-3'>
                {exportJobs.length === 0 ? (
                  <div className='text-center py-8 text-muted-foreground'>
                    <Download className='h-8 w-8 mx-auto mb-2 opacity-50' />
                    <p>No export jobs yet</p>
                  </div>
                ) : (
                  exportJobs.map(job => (
                    <div key={job.id} className='p-3 border rounded-lg'>
                      <div className='flex items-center justify-between mb-2'>
                        <div className='flex items-center space-x-2'>
                          {getStatusIcon(job.status)}
                          <span className='font-medium text-sm truncate'>{job.fileName}</span>
                        </div>
                        <Badge className={getStatusColor(job.status)}>{job.status}</Badge>
                      </div>

                      {job.status === 'processing' && (
                        <div className='space-y-2'>
                          <Progress value={job.progress} className='h-2' />
                          <p className='text-xs text-muted-foreground'>
                            {job.progress.toFixed(0)}% complete
                          </p>
                        </div>
                      )}

                      {job.status === 'completed' && job.downloadUrl && (
                        <Button
                          size='sm'
                          onClick={() => downloadExport(job)}
                          className='w-full mt-2'
                        >
                          <Download className='h-4 w-4 mr-2' />
                          Download
                        </Button>
                      )}

                      {job.status === 'failed' && job.errorMessage && (
                        <Alert variant='destructive' className='mt-2'>
                          <AlertDescription className='text-xs'>
                            {job.errorMessage}
                          </AlertDescription>
                        </Alert>
                      )}

                      <p className='text-xs text-muted-foreground mt-2'>
                        Created: {new Date(job.createdAt).toLocaleString()}
                      </p>
                    </div>
                  ))
                )}
              </div>
            </CardContent>
          </Card>

          {/* Quick Actions */}
          <Card>
            <CardHeader>
              <CardTitle>Quick Actions</CardTitle>
            </CardHeader>
            <CardContent className='space-y-2'>
              <Button variant='outline' size='sm' className='w-full'>
                <Share2 className='h-4 w-4 mr-2' />
                Share Analysis
              </Button>
              <Button variant='outline' size='sm' className='w-full'>
                <Mail className='h-4 w-4 mr-2' />
                Email Report
              </Button>
              <Button variant='outline' size='sm' className='w-full'>
                <Printer className='h-4 w-4 mr-2' />
                Print Summary
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
};
