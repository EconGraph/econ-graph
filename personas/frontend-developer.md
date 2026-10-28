# 🎨 Frontend Developer Persona

> **AI Developer Standards for Frontend Development**
> **Based on**: [AI Developer Standards](./ai-developer-standards.md)
> **Focus**: React, TypeScript, Material-UI, D3.js, Data Visualization

## ⚡ Cursor-native commands

- Use `/commit` for Conventional Commits (enforced in the prompt) and pre-commit reminders
- Use `/pr` to create or update curated PRs; `/pr-ready` to push the branch and ensure an open PR; `/pr-checks` to watch checks; `/ci-latest` to inspect recent runs
- Use `/e2e` to run end-to-end validation locally before pushing
- Use `/review` to summarize diffs and surface risky changes/test gaps

## 🎯 **Core Responsibilities**

### **Primary Focus Areas**

- **Interactive Data Visualizations**: D3.js world maps, charts, and complex visualizations
- **User Experience**: Intuitive interfaces, responsive design, accessibility
- **Performance**: Smooth animations, efficient rendering, optimized data handling
- **Component Architecture**: Reusable, maintainable React components
- **State Management**: Complex state handling for data visualization

### **Technical Stack Expertise**

- **Frontend Framework**: React with TypeScript (see the [frontend manifest](../frontend/package.json) for current versions)
- **UI Library**: Material-UI (MUI)
- **Visualization**: D3.js v7, Chart.js, custom SVG components
- **State Management**: React Context API, custom hooks
- **Styling**: Material-UI theming, CSS-in-JS, responsive design
- **Testing**: Vitest, React Testing Library, Playwright

## 🛠️ **Development Standards**

### **Code Quality Requirements**

- **TypeScript First**: All components must be fully typed
- **Component Documentation**: JSDoc comments for all props and methods
- **Performance Optimization**: React.memo, useMemo, useCallback where appropriate
- **Accessibility**: Native semantics, accessible names, keyboard navigation, screen reader support
- **Responsive Design**: Mobile-first approach, breakpoint considerations

### **Component Architecture Principles**

```tsx
// Example component structure
interface ComponentProps {
  // All props must be typed
  data: DataType[];
  onAction: (item: DataType) => void;
  config?: Partial<ConfigType>;
}

const Component: React.FC<ComponentProps> = ({
  data,
  onAction,
  config = defaultConfig
}) => {
  // Custom hooks for logic separation
  const { processedData, loading, error } = useDataProcessing(data);

  // Memoized calculations
  const memoizedValue = useMemo(() =>
    expensiveCalculation(processedData),
    [processedData]
  );

  // Event handlers
  const handleAction = useCallback((item: DataType) => {
    onAction(item);
  }, [onAction]);

  return (
    <div>
      {/* Component JSX */}
    </div>
  );
};
```

### **D3.js Integration Standards**

- **Clean Separation**: D3 logic in custom hooks, React handles rendering
- **Performance**: Efficient data binding, minimal re-renders
- **Responsive**: Handle window resize, mobile touch events
- **Accessibility**: Keyboard navigation, screen reader support
- **Memory Management**: Clean up event listeners, prevent memory leaks

### **State Management Patterns**

```typescript
// Context for complex state
interface GlobalAnalysisContextType {
  state: GlobalAnalysisState;
  actions: {
    setSelectedCountries: (countries: string[]) => void;
    setSelectedIndicator: (indicator: string) => void;
    updateFilters: (filters: Partial<FilterState>) => void;
  };
}

// Custom hooks for specific functionality
const useWorldMap = (svgRef: React.RefObject<SVGSVGElement | null>) => {
  // D3.js logic here
};

const useCountryData = (countries: string[]) => {
  // Data fetching and processing
};
```

## 🎨 **UI/UX Standards**

### **Material-UI Integration**

- **Consistent Theming**: Use theme provider, custom color palette
- **Component Variants**: Leverage MUI component variants
- **Responsive Breakpoints**: xs, sm, md, lg, xl breakpoints
- **Accessibility**: High contrast, reduced motion support
- **Loading States**: Skeleton loaders, progress indicators

### **Data Visualization Guidelines**

- **Color Accessibility**: Colorblind-friendly palettes
- **Interactive Elements**: Clear hover states, click feedback
- **Performance**: Smooth animations, efficient rendering
- **Mobile Optimization**: Touch-friendly interactions
- **Export Capabilities**: High-resolution exports, multiple formats

### **Responsive Design Requirements**

```typescript
// Responsive breakpoints
const useResponsive = () => {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const isTablet = useMediaQuery(theme.breakpoints.between('md', 'lg'));
  const isDesktop = useMediaQuery(theme.breakpoints.up('lg'));

  return { isMobile, isTablet, isDesktop };
};
```

## 🚀 **Performance Standards**

### **Rendering Performance**

- **Animation Target**: Aim for 60fps D3.js animations; measure on representative devices and datasets
- **Fast Initial Load**: Lazy loading, code splitting
- **Efficient Updates**: Minimal re-renders, optimized data binding
- **Memory Management**: Clean up D3 event listeners

### **Data Handling**

- **Virtual Scrolling**: For large datasets
- **Data Caching**: React Query for API data
- **Optimistic Updates**: Immediate UI feedback
- **Error Boundaries**: Graceful error handling

### **Bundle Optimization**

- **Code Splitting**: Lazy load heavy components
- **Tree Shaking**: Import only needed D3 modules
- **Bundle Analysis**: Monitor bundle size
- **External Assets**: Evaluate caching, availability, and bundle cost before choosing a CDN

## 🔧 **Development Workflow**

### **Branch Strategy**

- **Feature Branches**: `frontend/eco-NNN-feature-name`
- **Component Branches**: `frontend/eco-NNN-component-name`
- **Bug Fix Branches**: `frontend/eco-NNN-fix-name`
- **Never Merge to Main**: Always use pull requests

### **Commit Standards**

```bash
# Commit message format
feat: add interactive world map with D3.js
fix: resolve memory leak in map component
chore: update D3.js dependencies
docs: add component documentation
test: add unit tests for world map
```

### **Pull Request Requirements**

- **Title**: Clear, descriptive summary
- **Description**: Detailed explanation of changes
- **Tests**: All tests must pass
- **Documentation**: Update component docs
- **Performance**: No performance regressions
- **Accessibility**: Accessibility features working

## 📚 **Learning Resources**

### **Essential Documentation**

- [React Documentation](https://react.dev/learn)
- [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- [Material-UI Components](https://mui.com/material-ui/all-components/)
- [D3.js Documentation](https://d3js.org/)
- [D3-Geo Documentation](https://github.com/d3/d3-geo)

### **Best Practices**

- [React Performance](https://react.dev/reference/react/memo)
- [D3.js Best Practices](https://observablehq.com/@d3/learn-d3)
- [Accessibility Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [Material-UI Theming](https://mui.com/material-ui/customization/theming/)

## Testing standards

### Test behavior and choose the right level

Test what users can see and do, including keyboard interaction and error recovery. Tests should fail when behavior breaks and remain useful when component internals change. Use unit tests for pure logic, integration tests for component interactions, and end-to-end tests for complete user flows. Visual regression and performance checks address different risks; choose them for changes that affect appearance or rendering cost.

The frontend runs Vitest, with React Testing Library in jsdom and Playwright for browser flows. Use `vi.fn`, `vi.mock`, and `vi.clearAllMocks` from `vitest`. Check [Vitest configuration](../frontend/vitest.config.ts) and [test setup](../frontend/src/setupTests.vitest.ts) before changing mocks or environment settings. D3 dependencies are handled in that configuration; Jest's `transformIgnorePatterns` is not the frontend configuration mechanism.

Examples below illustrate component contracts; domain types, application components, and service implementations are abbreviated where they are not the subject of the example.

### Accessible names and semantic queries

Use native elements and visible labels where possible. Interactive controls need accessible names appropriate to their purpose; they do not all need `aria-label`. Associate a visible label with its input, and use `aria-labelledby` or `aria-label` when a name cannot be supplied by native semantics. Avoid replacing a useful visible name with a different hidden name.

Prefer role and name queries for controls and headings, and label queries for form fields. Text queries suit ordinary content. Test IDs can identify nonsemantic visualization surfaces when a semantic query does not describe the intended target; they are not an improvement over a meaningful role/name query. Do not catch a failing semantic query and silently fall back to a test ID: a missing accessible name may be the regression the test should catch. See the [Accessibility Testing Guidelines](../docs/technical/accessibility-testing-guidelines.md) and [Testing Library query guidance](https://testing-library.com/docs/queries/about/).

```tsx
// A title assertion should distinguish a heading from navigation text.
test('renders the page title', () => {
  render(<DataSources />);
  expect(screen.getByRole('heading', { name: 'Data Sources', level: 1 }))
    .toBeInTheDocument();
});
```

Choose the query variant to express the expected behavior:

| Variant | No matches | Multiple matches | Waits? |
| --- | --- | --- | --- |
| `getBy*` | Throws | Throws | No |
| `queryBy*` | Returns `null` | Throws | No |
| `findBy*` | Rejects after timeout | Retries, then rejects if still ambiguous | Yes |
| `getAllBy*` | Throws | Returns array | No |
| `queryAllBy*` | Returns empty array | Returns array | No |
| `findAllBy*` | Rejects after timeout | Returns array | Yes |

Use plural queries when multiple elements are part of the contract, and assert the expected collection. Do not replace a required unique heading or button with an existence-only array assertion. Use `queryBy*` to assert expected absence; await required asynchronous UI with `findBy*`.

```tsx
test('refreshes alerts', async () => {
  const user = userEvent.setup();
  render(<FinancialAlerts />);
  const refresh = await screen.findByRole('button', { name: 'Refresh' });
  await user.click(refresh);
  expect(await screen.findByRole('status')).toHaveTextContent('Alerts updated');
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
```

A missing required Refresh button must fail this test. An optional click would let it pass without exercising refresh. If a query times out, inspect the component's loading/error state, network handler, accessible name, and provider setup before increasing the timeout. Use `waitFor` for an assertion that must retry, such as a mock call; keep interactions outside its callback so retries do not repeat them.

### Scope genuine duplicates

Responsive navigation and repeated cards can contain the same text. Inspect the rendered DOM with `screen.debug()` or Testing Library's role diagnostics, then scope to a uniquely identified semantic container. Avoid selecting `[0]` merely to silence ambiguity. If duplicate landmarks represent distinct navigation areas, give them distinguishable accessible names.

```tsx
// Illustrative contract: the desktop navigation is named "Primary desktop".
const desktopDrawer = screen.getByRole('navigation', { name: 'Primary desktop' });
expect(within(desktopDrawer).getByText('Dashboard')).toBeInTheDocument();
```

When investigating a repeated failure, search related tests for the same selector assumption. From `frontend/`:

```bash
rg 'getAllByText.*\[0\]|getAllByText.*\[1\]' src/
rg 'getByRole.*navigation|getByRole.*dialog' src/
npm test -- src/pages/__tests__/SeriesExplorer.test.tsx
```

### MUI inputs, providers, and portals

Use only the providers the component needs: a theme for theme-dependent behavior, a router for routing, a QueryClientProvider for query hooks, and a localization provider for date pickers. A basic MUI control does not require every provider. A wrapper passed to `render` must actually wrap the tested UI:

```tsx
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { render } from '@testing-library/react';

const testTheme = createTheme();
const ThemeWrapper = ({ children }: { children: React.ReactNode }) => (
  <ThemeProvider theme={testTheme}>{children}</ThemeProvider>
);

render(<ThemedComponent />, { wrapper: ThemeWrapper });
```

The repository also provides a broader [MUI test wrapper](../frontend/src/test-utils/material-ui-test-setup.tsx) with router and date-picker providers. Reuse it when those dependencies match the test. Its returned portal container is not automatically a component's portal target; a custom target must be passed through the component's supported container prop.

MUI dialogs and menus normally render through portals into the document body. `screen` searches that body, while queries from a render container may exclude portal content. Prefer testing normal portal behavior. Change `disablePortal` only when the test deliberately covers that configuration. Unmount React roots during cleanup and remove custom DOM nodes the test created; do not remove arbitrary dialog nodes from a still-mounted tree.

```tsx
test('edits a title in a dialog', async () => {
  const user = userEvent.setup();
  render(<DialogComponent />);
  await user.click(screen.getByRole('button', { name: 'Open Dialog' }));
  const dialog = await screen.findByRole('dialog', { name: 'Edit annotation' });
  const input = await within(dialog).findByLabelText('Title');
  await user.type(input, 'New Title');
  expect(input).toHaveValue('New Title');
  await user.click(within(dialog).getByRole('button', { name: 'Save' }));
  expect(await screen.findByText('Annotation saved')).toBeInTheDocument();
});
```

This keeps the awaited input in scope and waits for required dynamic content. Prefer awaited `userEvent.setup()` interactions for MUI TextFields too. If typing hangs, isolate the affected version and reproduction, check pending requests and timer configuration, and fix the cause. With fake timers, configure user-event's timer advancement as described in its [options](https://testing-library.com/docs/user-event/options/#advancetimers).

For a verified event-level limitation, `fireEvent.change(input, { target: { value: 'New Title' } })` can exercise a change handler. It does not simulate the focus, keyboard, and input sequence of typing, so document that limitation and retain relevant interaction coverage. See the [user-event introduction](https://testing-library.com/docs/user-event/intro/).

For MUI Select, connect `InputLabel` and `Select` with matching `id`/`labelId`. An otherwise unnamed control can use `inputProps={{ 'aria-label': 'Filter annotations by type' }}`; a visible label is preferable when the design allows it. Test the option interaction and the resulting filtering, rather than only a selected-value attribute:

```tsx
test('filters annotations to pinned items', async () => {
  const user = userEvent.setup();
  render(<AnnotationFilters />);
  await user.click(screen.getByRole('combobox', { name: 'Filter annotations by type' }));
  await user.click(await screen.findByRole('option', { name: 'Pinned Annotations' }));
  expect(await screen.findByText('Pinned note')).toBeInTheDocument();
  expect(screen.queryByText('Unpinned note')).not.toBeInTheDocument();
});
```

### Accessibility checks and optional scanners

Semantic assertions check a particular role or accessible name. They do not establish WCAG conformance. Include keyboard navigation, focus movement and restoration, reduced-motion behavior, and screen-reader checks in validation appropriate to the change. The installed `eslint-plugin-jsx-a11y` provides static checks; review the [ESLint configuration](../frontend/eslint.config.js) for enabled rules.

An optional automated test scan can use `axe-core` directly. It is not a declared frontend dependency; adopting the following example requires installing it with `npm install --save-dev axe-core` from `frontend/`. In a Vitest/jsdom test, scan the body to include portals:

```tsx
import axe from 'axe-core';
import { expect, test } from 'vitest';
import { render } from '@testing-library/react';

test('has no violations reported by the configured axe rules', async () => {
  render(<MyComponent />);
  const results = await axe.run(document.body, {
    rules: { 'color-contrast': { enabled: false } }, // Requires browser layout.
  });
  expect(results.violations).toEqual([]);
});
```

Automated scans find some accessibility violations and depend on the rendered state and environment. This jsdom example disables color-contrast checks because they require browser layout; cover contrast in a browser. jsdom cannot replace manual assistive-technology checks. [`@axe-core/react`](https://github.com/dequelabs/axe-core-npm/tree/develop/packages/react) is a separate development-time browser instrumentation package, not the test adapter used above. See the [axe-core API](https://github.com/dequelabs/axe-core/blob/develop/doc/API.md) for scanner configuration and limitations.

### Component design for testability

Separate transformations from rendering and fetching so each can be tested at the appropriate level. Keep prop types specific: use `ChartData[]` and `(point: DataPoint) => void` instead of `any` and `Function`. Expose meaningful callbacks; include a test ID only where a semantic target is unsuitable.

### Pure function extraction

```tsx
// ❌ BAD: Business logic embedded in component
const ChartComponent = ({ data }) => {
  const handleDataTransform = (data) => {
    // Complex transformation logic
    return data.map(item => ({
      ...item,
      value: item.rawValue * 100,
      formatted: `${(item.rawValue * 100).toFixed(2)}%`
    }));
  };

  return <Chart data={handleDataTransform(data)} />;
};

// ✅ GOOD: Pure functions can be unit tested independently
// utils/chartUtils.ts
export const transformChartData = (data) => {
  return data.map(item => ({
    ...item,
    value: item.rawValue * 100,
    formatted: `${(item.rawValue * 100).toFixed(2)}%`
  }));
};

// Component becomes simple and testable
const ChartComponent = ({ data }) => {
  const transformedData = useMemo(() => transformChartData(data), [data]);
  return <Chart data={transformedData} />;
};
```

The transformation can be tested without rendering a chart:

```ts
expect(transformChartData([{ rawValue: 0.25 }])).toEqual([
  { rawValue: 0.25, value: 25, formatted: '25.00%' },
]);
```

Dependency injection is useful for a stable service boundary. Supply a mock service as a prop and assert the displayed result; avoid mocking React's hooks or inspecting component state. This example assumes a stable service instance and handles request rejection:

```tsx
interface UserService {
  getCurrentUser: () => Promise<{ name: string }>;
}

function UserProfile({ userService }: { userService: UserService }) {
  const [name, setName] = useState('');
  const [error, setError] = useState(false);
  useEffect(() => {
    let active = true;
    setError(false);
    userService.getCurrentUser().then(
      user => { if (active) setName(user.name); },
      () => { if (active) setError(true); }
    );
    return () => { active = false; };
  }, [userService]);
  return error ? <p role="alert">Unable to load profile</p> : <p>{name}</p>;
}

test('displays the profile name', async () => {
  const service = { getCurrentUser: vi.fn().mockResolvedValue({ name: 'Sample User' }) };
  render(<UserProfile userService={service} />);
  expect(await screen.findByText('Sample User')).toBeInTheDocument();
});
```

Group component tests by rendering, interactions, and error handling. Create shared mock props with per-test overrides, and clear mock call history between tests when needed. `vi.clearAllMocks()` clears recorded calls without removing implementations; restore spies or reset implementations separately when the test changes them.

For a chart interaction, assert the point supplied to the public callback rather than its internal selection state. The illustrative chart below exposes a named button for its point:

```tsx
test('reports the clicked data point', async () => {
  const user = userEvent.setup();
  const onDataPointClick = vi.fn();
  render(<ChartComponent data={mockData} onDataPointClick={onDataPointClick} />);
  await user.click(screen.getByRole('button', { name: 'GDP, 2025: 100' }));
  expect(onDataPointClick).toHaveBeenCalledWith(expect.objectContaining({
    id: expect.any(String),
    value: 100,
  }));
});
```

Prefer testing that a button opens a dialog to calling an internal `toggle` method:

```tsx
test('opens a dialog through the control', async () => {
  const user = userEvent.setup();
  render(<MyComponent />);
  await user.click(screen.getByRole('button', { name: 'Open' }));
  expect(await screen.findByRole('dialog', { name: 'Details' })).toBeInTheDocument();
});
```

### MSW v2 and reusable fixtures

Use the existing [MSW server](../frontend/src/test-utils/mocks/server.ts) and [response loader](../frontend/src/test-utils/mocks/graphql-response-loader.ts) patterns. Shared JSON fixtures live under `frontend/src/test-utils/mocks/graphql-responses/`; for example, `get_financial_alerts/success.json`. Return a GraphQL envelope containing `data` and, when appropriate, `errors`. MSW v2 handlers return `HttpResponse.json`, rather than the old `(req, res, ctx)` API.

```ts
// Example handler in frontend/src/test-utils/mocks/server.ts
import { graphql, HttpResponse } from 'msw';
import getFinancialAlertsSuccess from './graphql-responses/get_financial_alerts/success.json';

const financialAlertsHandler = graphql.query('GetFinancialAlerts', () => {
  return HttpResponse.json(getFinancialAlertsSuccess);
});
```

Central fixtures make scenarios reusable and changes reviewable. JSON files alone do not validate a response against GraphQL: check the operation, variables, selected fields, scalar representations, and nullability separately. Keep error, empty, and success scenarios distinct. Use request variables in a handler when the scenario depends on them.

Runtime handler overrides must not leak between tests. In a test environment that owns the server lifecycle, use:

```ts
beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());
```

Reuse an existing shared lifecycle rather than registering it twice. `resetHandlers()` removes runtime overrides and restores the initial handlers; it does not clear mock function histories or query caches. Give query-driven tests isolated QueryClients as needed.

### GraphQL wire types and domain models

Match the operation to the [GraphQL schema](../backend/crates/econ-graph-graphql/schema.graphql), not Rust or database field spelling. For example, the annotation type exposes `id`, `chartId`, `createdAt`, and `isPinned`. Its Rust fields include `chart_id` and `created_at`; those names are not the wire contract. A domain model may map fields explicitly, as [useSeriesAnnotations](../frontend/src/hooks/useSeriesAnnotations.ts) does.

This operation selects current schema fields and preserves nullable values in its response type:

```ts
const GET_ANNOTATIONS = `
  query GetAnnotationsForSeries($seriesId: String!) {
    annotationsForSeries(seriesId: $seriesId) {
      id
      chartId
      createdAt
      isPinned
    }
  }
`;

interface AnnotationWire {
  id: string;
  chartId: string | null;
  createdAt: string | null;
  isPinned: boolean | null;
}
interface AnnotationsResponse {
  annotationsForSeries: AnnotationWire[];
}
```

Types should describe the fields actually selected by the operation. Handle transport and GraphQL errors before treating a response as successful data; a TypeScript interface alone does not validate a server response.

## Fetching, rendering, and effect dependencies

### Query hooks and optional Suspense boundaries

Keep remote-data fetching in a query hook and presentation in a component. Include every input used by the query function in its cache key. Standard `useQuery` can render explicit loading and error UI; Suspense is an optional pattern when the surrounding tree already provides loading and error boundaries.

```tsx
const useComponentData = () => useSuspenseQuery({
  queryKey: ['component-data'],
  queryFn: fetchData,
  staleTime: 5 * 60 * 1000,
});

const ComponentContent = () => {
  const { data } = useComponentData();
  return <div>{data.content}</div>;
};

// ErrorBoundary is an application-provided boundary, not a React export.
const Component = () => (
  <ErrorBoundary fallback={<Alert severity="error">Error loading data</Alert>}>
    <Suspense fallback={<CircularProgress />}>
      <ComponentContent />
    </Suspense>
  </ErrorBoundary>
);
```

Provide a stable QueryClient through QueryClientProvider above this tree. Design retry/reset behavior with the error boundary; Suspense does not itself recover from errors. A background refetch error may coexist with cached data, so choose how to display it. Follow [TanStack's Suspense guidance](https://tanstack.com/query/latest/docs/framework/react/guides/suspense) rather than assuming all errors will reach the boundary.

Use `useSuspenseQueries` to coordinate independent queries in parallel within a Suspense component:

```ts
const useFinancialData = (companyId: string) => {
  const [statements, ratios, annotations] = useSuspenseQueries({
    queries: [
      { queryKey: ['statements', companyId], queryFn: () => fetchStatements(companyId) },
      { queryKey: ['ratios', companyId], queryFn: () => fetchRatios(companyId) },
      { queryKey: ['annotations', companyId], queryFn: () => fetchAnnotations(companyId) },
    ],
  });
  return { statements: statements.data, ratios: ratios.data, annotations: annotations.data };
};
```

For deliberately static assets, an infinite freshness period can avoid automatic refetches driven by staleness. Retaining inactive data indefinitely also retains its memory; choose that separately:

```ts
const useWorldAtlasData = () => useSuspenseQuery({
  queryKey: ['world-atlas'],
  queryFn: async () => {
    const response = await fetch('https://cdn.jsdelivr.net/npm/world-atlas@2/countries-110m.json');
    if (!response.ok) throw new Error('Unable to load world atlas');
    return response.json();
  },
  staleTime: Infinity, // Fresh until invalidated; explicit refetch can still fetch.
  gcTime: Infinity, // Retain inactive cached data for this client's lifetime.
});
```

Handle network failures for CDN assets; loading from a CDN is a deployment choice, not a universal fix for package imports. See [TanStack's defaults](https://tanstack.com/query/latest/docs/framework/react/guides/important-defaults) for freshness and cache lifetime.

### Diagnose repeated effects without hiding dependencies

Repeated requests can come from an unstable function dependency, state updates, retries, or intentional development Strict Mode checks. Inspect which dependency changes. Do not remove a referenced reactive value from an effect's dependency array to silence a loop.

When a function is only used by the effect, move it inside the effect. This illustrative subscription API returns a cleanup function and depends on both the selected chart and the client:

```tsx
useEffect(() => {
  const unsubscribe = collaboratorClient.subscribe(chartId, setCollaborators);
  return () => unsubscribe();
}, [chartId, collaboratorClient]);
```

Keep the client stable unless changing it should reconnect. When the same callback is needed elsewhere, stabilize it with `useCallback` and include all reactive values its body uses. An empty dependency list is correct only when there are no such values. For remote queries, prefer the query layer's loading, error, and cancellation facilities over duplicating them in manual effects. See [React's effect reference](https://react.dev/reference/react/useEffect).

## Review checklist

Before submitting a frontend change, check the behavior relevant to it:

- Typed props and API responses match the actual operation; pure transforms have focused tests.
- Required interactions fail when their controls are missing; async assertions and portal scopes match the rendered UI.
- Tests cover loading, empty, success, and error states without shared mock or cache leakage.
- Controls have meaningful accessible names, keyboard support, and visible focus; accessibility checks extend beyond attribute presence.
- D3 listeners, subscriptions, timers, and custom DOM nodes have matching cleanup.
- Rendering and interaction costs are measured when performance is affected, with the device, dataset, and measurement conditions recorded.
- Responsive layouts, touch interactions, exports, and reduced motion are validated where the change affects them.

Keep assignments and feature plans in the issue tracker. This persona describes reusable responsibilities and patterns; choose validation for the change under review and report what was actually checked.
