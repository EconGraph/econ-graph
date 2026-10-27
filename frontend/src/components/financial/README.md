# Financial UI Components

This directory contains React components for exploring financial statements,
ratios, trends, and educational explanations. The component catalog below helps
developers choose views and supply their data and callbacks.

The components are used by the financial showcase, which is excluded from both
committed development and release build profiles by the `financial_components`
flag. Some views query financial data; others render supplied data or demonstrate
controls with sample values and simulated actions. See [Data Integration](#-data-integration)
for caller requirements and [Demo Page](#-demo-page) for preview instructions.

## 📁 Component Overview

### Core Components

#### 1. **FinancialDashboard**

**File:** [FinancialDashboard.tsx](FinancialDashboard.tsx)

The main dashboard queries a company's overview, statements, and ratios by
`companyId`.

**Features:**

- Company overview with key metrics
- Recent filings display
- Financial ratio summaries
- Tabbed navigation (Overview, Statements, Ratios, Trends, Comparison, Analysis)
- Loading and error displays for the dashboard query

Trends, Comparison, and Analysis tabs show descriptive placeholders. The Refresh,
Download, and Share buttons have no action handlers.

**Usage:**

```tsx
import { FinancialDashboard } from '@/components/financial';

<FinancialDashboard
  companyId='0000320193'
  userType='intermediate'
  showEducationalContent={true}
  showCollaborativeFeatures={true}
/>;
```

#### 2. **FinancialStatementViewer**

**File:** [FinancialStatementViewer.tsx](FinancialStatementViewer.tsx)

Detailed viewer for individual financial statements with interactive features.

**Features:**

- Tabbed interface (Statement, Ratios, Analysis, Education)
- Line item details with trend indicators
- Query-backed annotation display
- Educational content integration
- Suspense loading fallback

Statement and annotation query failures propagate to the nearest host error
boundary. Wrap the viewer in an error boundary to provide a contained failure
display.

Annotation creation and subscriptions use no-op stand-ins, and the edit/delete
callbacks are empty. Enabling `showCollaborativeFeatures` does not connect a live
collaboration service.

**Usage:**

```tsx
import { FinancialStatementViewer } from '@/components/financial';

<FinancialStatementViewer
  statementId='statement-123'
  companyId='0000320193'
  userType='intermediate'
  showEducationalContent={true}
  showCollaborativeFeatures={true}
/>;
```

#### 3. **TrendAnalysisChart**

**File:** [TrendAnalysisChart.tsx](TrendAnalysisChart.tsx)

Trend selection and calculations for caller-supplied financial ratios.

**Features:**

- Time range selection (1Y, 3Y, 5Y, 10Y)
- Line/bar chart type selection
- Ratio selection and filtering
- Trend calculations with strength indicators
- Four-quarter projections that repeat the difference between the last two values

The chart area is a placeholder; selecting a chart type changes local state.
The Export Chart button has no handler. `statements` is accepted but not used in
the calculations. The caller controls `timeRange` and updates it through
`onTimeRangeChange`.

**Usage:**

```tsx
import { TrendAnalysisChart } from '@/components/financial';

<TrendAnalysisChart
  ratios={ratios}
  statements={statements}
  timeRange='3Y'
  onTimeRangeChange={setTimeRange}
/>;
```

#### 4. **PeerComparisonChart**

**File:** [PeerComparisonChart.tsx](PeerComparisonChart.tsx)

Compare financial metrics with industry peers and competitors.

**Features:**

- Peer-company query combined with the supplied current company and ratios
- Table view with ratio values and percentile indicators
- Industry filtering and sorting by name, performance score, or market cap
- Table/chart/radar view selection

Chart and radar views render placeholders. The performance summary contains fixed
sample values, including its overall ranking, industry, and market cap; it is not
a calculated summary of the supplied company. The Export Comparison button has
an empty handler.

**Usage:**

```tsx
import { PeerComparisonChart } from '@/components/financial';

<PeerComparisonChart
  ratios={ratios}
  company={company}
  userType='intermediate'
/>;
```

#### 5. **FinancialAlerts**

**File:** [FinancialAlerts.tsx](FinancialAlerts.tsx)

A query-backed alert list with local filtering and alert actions.

**Features:**

- Display of alert types, including ratio thresholds, trend changes, and filing deadlines
- Severity levels (low, medium, high, critical)
- Filtering and sorting capabilities
- Mark-as-read and active/inactive controls

Alert actions update local component state without saving changes to the server.
There is no WebSocket or subscription connection. `ratios` and `statements` are
accepted but do not generate alerts; the list comes from the company alert query.

**Usage:**

```tsx
import { FinancialAlerts } from '@/components/financial';

<FinancialAlerts
  companyId='0000320193'
  ratios={ratios}
  statements={statements}
  userType='intermediate'
  onAlertClick={handleAlertClick}
/>;
```

#### 6. **FinancialExport**

**File:** [FinancialExport.tsx](FinancialExport.tsx)

Export option selection and a simulated export-job interface.

**Features:**

- Format choices (PDF, Excel, CSV, JSON, PNG)
- Content customization (charts, raw data, annotations, benchmarks)
- Data selection (statements, ratios)
- Chart type selection
- Simulated export-job tracking with progress indicators

Progress is driven by timers. Completed jobs receive a constructed download URL,
but no file is generated and the download handler does nothing.

**Usage:**

```tsx
import { FinancialExport } from '@/components/financial';

<FinancialExport
  company={company}
  statements={statements}
  ratios={ratios}
  userType='intermediate'
/>;
```

#### 7. **FinancialMobile**

**File:** [FinancialMobile.tsx](FinancialMobile.tsx)

Mobile-responsive wrapper that adapts components for different screen sizes.

**Features:**

- Layout selection by viewport width (mobile, tablet, desktop)
- Mobile-optimized navigation
- Touch-friendly interfaces
- Responsive layouts
- Statement selection and tab navigation

The wrapper passes company and statement identifiers to nested query-backed
components; supplying arrays does not replace those requests. Some visible status
and change indicators use fixed sample values.

**Usage:**

```tsx
import { FinancialMobile } from '@/components/financial';

<FinancialMobile
  company={company}
  statements={statements}
  ratios={ratios}
  userType='intermediate'
  showEducationalContent={true}
  showCollaborativeFeatures={true}
/>;
```

### Supporting Components

#### 8. **RatioAnalysisPanel**

**File:** [RatioAnalysisPanel.tsx](RatioAnalysisPanel.tsx)

Queries ratios by `statementId` and groups them for analysis with educational
content.

#### 9. **BenchmarkComparison**

**File:** [BenchmarkComparison.tsx](BenchmarkComparison.tsx)

Displays the supplied company value alongside optional industry benchmark data,
including percentiles and the industry median.

#### 10. **AnnotationPanel**

**File:** [AnnotationPanel.tsx](AnnotationPanel.tsx)

Displays supplied annotations and invokes caller-provided edit/delete callbacks.
The caller owns persistence; the add callback is accepted but not used by the panel.

#### 11. **EducationalPanel**

**File:** [EducationalPanel.tsx](EducationalPanel.tsx)

Displays a supplied ratio name, formula, description, and optional learning link.

#### 12. **CollaborativePresence**

**File:** [CollaborativePresence.tsx](CollaborativePresence.tsx)

Displays caller-supplied team members and the current user. Presence discovery
and synchronization belong to the caller.

#### 13. **RatioExplanationModal**

**File:** [RatioExplanationModal.tsx](RatioExplanationModal.tsx)

Modal component for explaining financial ratios and concepts.

## 🎨 Design System

### User Types

The `userType` prop accepts `beginner`, `intermediate`, `advanced`, or `expert`.
Its effect depends on the component; these values do not define permission or
feature tiers. For example, `RatioAnalysisPanel` shows an additional explanation
for beginners when `showEducationalContent` is true. `FinancialDashboard`,
`PeerComparisonChart`, `FinancialAlerts`, and `FinancialExport` accept `userType`
without changing their behavior.

### Responsive Design

`FinancialMobile` selects its layout from the viewport width on mount and resize:

- **Mobile** (< 768px): Bottom navigation and drawer menus
- **Tablet** (768px ≤ width < 1024px): Tabbed views and a two-column metrics grid
- **Desktop** (≥ 1024px): Embedded financial dashboard

The wrapper does not track orientation separately. Other components use their own
responsive styles; these boundaries describe `FinancialMobile`.

### Color System

- **Success**: Green (positive trends, completed status)
- **Warning**: Yellow/Orange (caution, medium alerts)
- **Error**: Red (negative trends, critical alerts)
- **Info**: Blue (neutral information, links)
- **Muted**: Gray (secondary information, disabled states)

## 🔧 Technical Features

### State and Rendering

React hooks manage local selections, filters, and simulated jobs. Query-backed
components use TanStack Query and the project's `executeGraphQL` utility.
`TrendAnalysisChart`, `PeerComparisonChart`, and `FinancialAlerts` memoize derived
data, and several display components use `React.memo`.

### Accessibility

Controls include component-specific labels and state attributes. For example,
`PeerComparisonChart` labels its view switches and exposes their pressed state,
and `BenchmarkComparison` labels its benchmark region and values. Check keyboard
and screen-reader behavior in the host application when composing these views.

### Formatting

Formatting is defined in individual components. For example,
`FinancialStatementViewer` uses `en-US` number formatting and USD currency, while
`RatioAnalysisPanel` formats currencies with `en-US`. There is no shared locale or
RTL configuration exposed by this directory.

## 📊 Data Integration

### GraphQL Queries

`FinancialDashboard`, `FinancialStatementViewer`, `PeerComparisonChart`, and
`FinancialAlerts` fetch data using TanStack Query and
[`executeGraphQL`](../../utils/graphql.ts). Their identifiers must refer to data
available from the configured endpoint. The identifiers in this page's examples
illustrate prop shapes; they do not establish that those records exist.

`TrendAnalysisChart` and `FinancialExport` consume supplied props without making
queries. `AnnotationPanel`, `BenchmarkComparison`, `EducationalPanel`,
`CollaborativePresence`, and `RatioExplanationModal` also accept their display
data through props. `RatioAnalysisPanel` queries ratios by statement identifier.
`FinancialMobile` mixes supplied data with nested query-backed views.

The [barrel exports](index.ts) include the following query documents from
[`graphql/financial.ts`](../../graphql/financial.ts), along with annotation
mutation and subscription documents. Exporting a document does not connect a
component's inactive actions:

```typescript
// Get financial statements
GET_FINANCIAL_STATEMENTS;

// Get financial line items
GET_FINANCIAL_LINE_ITEMS;

// Get financial ratios
GET_FINANCIAL_RATIOS;

// Get company information
GET_COMPANY_INFO;

// Get annotations
GET_FINANCIAL_ANNOTATIONS;
```

### Type Safety

The [financial types](../../types/financial.ts) describe company, statement, ratio,
and annotation data. These abbreviated examples show their structure:

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

Components are already included in the project. Import them from the financial
components directory:

```typescript
import {
  FinancialDashboard,
  FinancialStatementViewer,
  TrendAnalysisChart,
  // ... other components
} from '@/components/financial';
```

### Basic Setup

The existing application provides `QueryClientProvider` in
[`src/index.tsx`](../../index.tsx), and authentication and theme contexts in
[`App.tsx`](../../App.tsx). Render query-backed components beneath the query
provider. A separate host must also supply the project's styles and suitable MUI
theme; see [`index.css`](../../index.css) and
[`ThemeContext.tsx`](../../contexts/ThemeContext.tsx).

The request utility defaults to `/graphql`, overridden at build time by
`VITE_GRAPHQL_URL`. Financial data queries use `VITE_DATA_GRAPHQL_URL` when set;
otherwise they use the same URL. Annotation operations use the app endpoint and
the host application's access-token integration. See the
[request utility](../../utils/graphql.ts) and
[authentication context](../../contexts/AuthContext.tsx) when integrating a
separate host. The endpoint must support the financial operations being rendered;
providing a query client alone does not supply financial records.

### Example Implementation

This page component runs beneath the existing application's providers:

```tsx
import React from 'react';
import { FinancialDashboard } from '@/components/financial';

const App: React.FC = () => {
  return (
    <FinancialDashboard
      companyId='0000320193'
      userType='intermediate'
      showEducationalContent={true}
      showCollaborativeFeatures={true}
    />
  );
};

export default App;
```

## 🧪 Testing

### Component Testing

Run the component tests in [`__tests__/`](__tests__/) from `frontend/`:

```bash
# Run component tests (Vitest filters by file path)
npm test -- financial

# Run with coverage
npm run test:coverage -- financial
```

### E2E Testing

The frontend also exposes the application-wide Playwright command. Run it from
`frontend/` with the environment required by the selected tests:

```bash
# Run E2E tests
npm run test:e2e
```

## 📱 Demo Page

The `/financial-components-demo` route is registered only when the
`financial_components` build flag is enabled. Both committed build profiles set
it to false, so starting the default development server does not expose the
route. Follow the [build-flag guide](../../../docs/build-flags.md) to configure a
build that includes it.

The [showcase page](../../pages/FinancialComponentsDemo.tsx) defines sample company,
statement, and ratio objects for its prop-driven views. Query-backed components
still request data by identifier; those sample objects do not intercept requests.
The page offers component selection, device preview widths, user-type selection,
and usage snippets. Its controls retain the simulations and placeholders
described in the catalog.

## 📚 Resources

### Support

- [GitHub Issues](https://github.com/EconGraph/econ-graph/issues)
- support@econgraph.com

---

_These components are part of the EconGraph project._
