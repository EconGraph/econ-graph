# Financial UI Components

This directory contains a comprehensive set of React components for financial statement visualization and analysis. These components provide a complete financial data analysis experience with modern UI/UX patterns, responsive design, and educational features.

## 📁 Component Overview

### Core Components

#### 1. **FinancialDashboard** 
**File:** `FinancialDashboard.tsx`

The main dashboard component that provides a comprehensive overview of a company's financial data.

**Features:**
- Company overview with key metrics
- Recent filings display
- Financial ratio summaries
- Tabbed navigation (Overview, Statements, Ratios, Trends, Comparison, Analysis)
- Real-time data updates
- Export and sharing capabilities

**Usage:**
```tsx
import { FinancialDashboard } from '@/components/financial';

<FinancialDashboard
  companyId="0000320193"
  userType="intermediate"
  showEducationalContent={true}
  showCollaborativeFeatures={true}
/>
```

#### 2. **FinancialStatementViewer**
**File:** `FinancialStatementViewer.tsx`

Detailed viewer for individual financial statements with interactive features.

**Features:**
- Tabbed interface (Statement, Ratios, Analysis, Education)
- Line item details with trend indicators
- Annotation system with real-time collaboration
- Educational content integration
- Progress tracking and error handling

**Usage:**
```tsx
import { FinancialStatementViewer } from '@/components/financial';

<FinancialStatementViewer
  statementId="statement-123"
  companyId="0000320193"
  userType="intermediate"
  showEducationalContent={true}
  showCollaborativeFeatures={true}
/>
```

#### 3. **TrendAnalysisChart**
**File:** `TrendAnalysisChart.tsx`

Interactive charts for analyzing financial trends over time.

**Features:**
- Time range selection (1Y, 3Y, 5Y, 10Y)
- Multiple chart types (line, bar)
- Ratio selection and filtering
- Trend calculations with strength indicators
- Forward-looking projections
- Export capabilities

**Usage:**
```tsx
import { TrendAnalysisChart } from '@/components/financial';

<TrendAnalysisChart
  ratios={ratios}
  statements={statements}
  timeRange="3Y"
  onTimeRangeChange={setTimeRange}
/>
```

#### 4. **PeerComparisonChart**
**File:** `PeerComparisonChart.tsx`

Compare financial metrics with industry peers and competitors.

**Features:**
- Multiple view modes (table, chart, radar)
- Industry filtering and sorting
- Performance scoring and rankings
- Percentile comparisons
- Interactive visualizations

**Usage:**
```tsx
import { PeerComparisonChart } from '@/components/financial';

<PeerComparisonChart
  ratios={ratios}
  company={company}
  userType="intermediate"
/>
```

#### 5. **FinancialAlerts**
**File:** `FinancialAlerts.tsx`

Real-time alerts and notifications system for financial metrics.

**Features:**
- Multiple alert types (ratio thresholds, trend changes, filing deadlines)
- Severity levels (low, medium, high, critical)
- Filtering and sorting capabilities
- Real-time updates with WebSocket integration
- Alert management (mark as read, toggle active)

**Usage:**
```tsx
import { FinancialAlerts } from '@/components/financial';

<FinancialAlerts
  companyId="0000320193"
  ratios={ratios}
  statements={statements}
  userType="intermediate"
  onAlertClick={handleAlertClick}
/>
```

#### 6. **FinancialExport**
**File:** `FinancialExport.tsx`

Comprehensive data export system supporting multiple formats.

**Features:**
- Multiple export formats (PDF, Excel, CSV, JSON, PNG)
- Content customization (charts, raw data, annotations, benchmarks)
- Data selection (statements, ratios)
- Chart type selection
- Export job tracking with progress indicators
- Download management

**Usage:**
```tsx
import { FinancialExport } from '@/components/financial';

<FinancialExport
  company={company}
  statements={statements}
  ratios={ratios}
  userType="intermediate"
/>
```

#### 7. **FinancialMobile**
**File:** `FinancialMobile.tsx`

Mobile-responsive wrapper that adapts components for different screen sizes.

**Features:**
- Device type detection (mobile, tablet, desktop)
- Orientation handling
- Mobile-optimized navigation
- Touch-friendly interfaces
- Responsive layouts
- Progressive disclosure

**Usage:**
```tsx
import { FinancialMobile } from '@/components/financial';

<FinancialMobile
  company={company}
  statements={statements}
  ratios={ratios}
  userType="intermediate"
  showEducationalContent={true}
  showCollaborativeFeatures={true}
/>
```

### Supporting Components

#### 8. **RatioAnalysisPanel**
**File:** `RatioAnalysisPanel.tsx`

Detailed analysis panel for financial ratios with educational content.

#### 9. **BenchmarkComparison**
**File:** `BenchmarkComparison.tsx`

Industry benchmarking and comparison tools.

#### 10. **AnnotationPanel**
**File:** `AnnotationPanel.tsx`

Collaborative annotation system for financial statements.

#### 11. **EducationalPanel**
**File:** `EducationalPanel.tsx`

Educational content integration for different user types.

#### 12. **CollaborativePresence**
**File:** `CollaborativePresence.tsx`

Real-time collaboration features and user presence indicators.

#### 13. **RatioExplanationModal**
**File:** `RatioExplanationModal.tsx`

Modal component for explaining financial ratios and concepts.

## 🎨 Design System

### User Types
Components support different user experience levels:

- **Beginner**: Simplified interfaces with educational content
- **Intermediate**: Balanced features with guided explanations
- **Advanced**: Full feature set with technical details
- **Expert**: Maximum functionality with minimal UI guidance

### Responsive Design
- **Mobile** (< 768px): Touch-optimized, bottom navigation, drawer menus
- **Tablet** (768px - 1024px): Hybrid interface with collapsible panels
- **Desktop** (> 1024px): Full dashboard with sidebars and detailed views

### Color System
- **Success**: Green (positive trends, completed status)
- **Warning**: Yellow/Orange (caution, medium alerts)
- **Error**: Red (negative trends, critical alerts)
- **Info**: Blue (neutral information, links)
- **Muted**: Gray (secondary information, disabled states)

## 🔧 Technical Features

### State Management
- React hooks for local state
- Context providers for shared state
- GraphQL integration with Apollo Client
- Real-time subscriptions for live updates

### Performance Optimizations
- Lazy loading for large datasets
- Virtual scrolling for long lists
- Memoization for expensive calculations
- Progressive image loading

### Accessibility
- ARIA labels and roles
- Keyboard navigation support
- Screen reader compatibility
- High contrast mode support
- Focus management

### Internationalization
- Multi-language support ready
- Currency and number formatting
- Date and time localization
- RTL language support

## 📊 Data Integration

### GraphQL Queries
All components use standardized GraphQL queries:

```typescript
// Get financial statements
GET_FINANCIAL_STATEMENTS

// Get financial line items
GET_FINANCIAL_LINE_ITEMS

// Get financial ratios
GET_FINANCIAL_RATIOS

// Get company information
GET_COMPANY_INFO

// Get annotations
GET_FINANCIAL_ANNOTATIONS
```

### Type Safety
Full TypeScript support with comprehensive type definitions:

```typescript
interface FinancialStatement {
  id: string;
  companyId: string;
  filingType: string;
  formType: string;
  // ... other properties
}

interface FinancialRatio {
  id: string;
  ratioName: string;
  value: number;
  category: string;
  // ... other properties
}
```

## 🚀 Getting Started

### Installation
Components are already included in the project. Import them from the financial components directory:

```typescript
import {
  FinancialDashboard,
  FinancialStatementViewer,
  TrendAnalysisChart,
  // ... other components
} from '@/components/financial';
```

### Basic Setup
1. Ensure GraphQL client is configured
2. Set up authentication context
3. Configure theme provider
4. Initialize Apollo Client with proper endpoints

### Example Implementation
```tsx
import React from 'react';
import { FinancialDashboard } from '@/components/financial';

const App: React.FC = () => {
  return (
    <FinancialDashboard
      companyId="0000320193"
      userType="intermediate"
      showEducationalContent={true}
      showCollaborativeFeatures={true}
    />
  );
};

export default App;
```

## 🧪 Testing

### Component Testing
Each component includes comprehensive tests:

```bash
# Run component tests
npm test -- --testPathPattern=financial

# Run with coverage
npm test -- --coverage --testPathPattern=financial
```

### E2E Testing
End-to-end tests for user workflows:

```bash
# Run E2E tests
npm run test:e2e
```

## 📱 Demo Page

Visit `/financial-components-demo` to see all components in action with:
- Interactive component showcase
- Device type switching (mobile, tablet, desktop)
- User type selection
- Live component preview
- Usage examples and code snippets

## 🔮 Future Enhancements

### Planned Features
- Advanced charting with D3.js integration
- AI-powered insights and recommendations
- Custom dashboard builder
- Advanced filtering and search
- Data visualization themes
- Offline support with service workers
- Progressive Web App (PWA) features

### Performance Improvements
- WebAssembly for heavy calculations
- Web Workers for data processing
- Advanced caching strategies
- Bundle optimization and code splitting

## 📚 Resources

### Documentation
- [Component API Reference](./API.md)
- [Design Guidelines](./DESIGN.md)
- [Performance Guide](./PERFORMANCE.md)
- [Accessibility Guide](./ACCESSIBILITY.md)

### Examples
- [Basic Usage Examples](./examples/)
- [Advanced Patterns](./examples/advanced/)
- [Integration Guides](./examples/integration/)

### Support
- [GitHub Issues](https://github.com/jmalicki/econ-graph/issues)
- [Documentation Site](https://docs.econ-graph.com)
- [Community Forum](https://community.econ-graph.com)

---

*This component library is part of the EconGraph project - a comprehensive financial data analysis platform.*
