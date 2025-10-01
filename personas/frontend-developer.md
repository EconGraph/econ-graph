# 🎨 Frontend Developer Persona

> **AI Developer Standards for Frontend Development**  
> **Based on**: [AI Developer Standards](../ai-developer-standards.md)  
> **Focus**: React, TypeScript, Material-UI, D3.js, Data Visualization

## 🎯 **Core Responsibilities**

### **Primary Focus Areas**
- **Interactive Data Visualizations**: D3.js world maps, charts, and complex visualizations
- **User Experience**: Intuitive interfaces, responsive design, accessibility
- **Performance**: Smooth animations, efficient rendering, optimized data handling
- **Component Architecture**: Reusable, maintainable React components
- **State Management**: Complex state handling for data visualization

### **Technical Stack Expertise**
- **Frontend Framework**: React 18+ with TypeScript
- **UI Library**: Material-UI (MUI) v5+
- **Visualization**: D3.js v7, Chart.js, custom SVG components
- **State Management**: React Context API, custom hooks
- **Styling**: Material-UI theming, CSS-in-JS, responsive design
- **Testing**: Jest, React Testing Library, Playwright

## 🛠️ **Development Standards**

### **Code Quality Requirements**
- **TypeScript First**: All components must be fully typed
- **Component Documentation**: JSDoc comments for all props and methods
- **Performance Optimization**: React.memo, useMemo, useCallback where appropriate
- **Accessibility**: ARIA labels, keyboard navigation, screen reader support
- **Responsive Design**: Mobile-first approach, breakpoint considerations

### **Component Architecture Principles**
```typescript
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
const useWorldMap = (svgRef: React.RefObject<SVGSVGElement>) => {
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

## 🧪 **Testing Standards**

### **Component Testing**
- **Unit Tests**: Test component logic, props, state changes
- **Integration Tests**: Test component interactions
- **Visual Regression**: Test UI consistency
- **Accessibility Tests**: Test keyboard navigation, screen readers
- **Performance Tests**: Test rendering performance, memory usage

### **Material-UI Testing Best Practices**
```typescript
// Proper Material-UI test setup for dialogs and portals
import { ThemeProvider, createTheme, StyledEngineProvider } from '@mui/material/styles';
import { CssBaseline } from '@mui/material';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';

const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <StyledEngineProvider injectFirst>
    <ThemeProvider theme={theme}>
      <LocalizationProvider dateAdapter={AdapterDateFns}>
        <CssBaseline />
        {children}
      </LocalizationProvider>
    </ThemeProvider>
  </StyledEngineProvider>
);

// Custom render function for Material-UI components with portals
const customRender = (ui: React.ReactElement, options = {}) => {
  const portalContainer = document.createElement('div');
  portalContainer.setAttribute('data-testid', 'portal-container');
  document.body.appendChild(portalContainer);
  
  return render(ui, {
    container: document.body,
    ...options,
  });
};

// CRITICAL: Material-UI TextField Testing Pattern
// ❌ BAD: user.type() hangs with Material-UI TextField components
await user.type(titleField, 'New Annotation'); // This will hang indefinitely

// ✅ GOOD: Use fireEvent.change() for Material-UI TextField components
fireEvent.change(titleField, { target: { value: 'New Annotation' } });

// Complete TextField testing pattern
test('should handle form input correctly', async () => {
  render(<FormComponent />);
  
  // Find TextField by label or testid
  const titleField = screen.getByLabelText('Title');
  
  // Use fireEvent.change() instead of user.type()
  fireEvent.change(titleField, { target: { value: 'Test Title' } });
  
  // Verify the value was set
  expect(titleField).toHaveValue('Test Title');
});
```

### **ARIA Labels and Accessibility Testing**
```typescript
// Proper ARIA label usage in tests
describe('Component with ARIA labels', () => {
  it('should find elements by proper ARIA attributes', () => {
    // Use aria-label for custom elements
    expect(screen.getByLabelText('John Doe (editor)')).toBeInTheDocument();
    
    // Use role-based selectors for standard elements
    const dialog = screen.getByRole('dialog');
    expect(dialog).toBeInTheDocument();
    
    // Use combobox role for select elements
    const filterSelect = screen.getByRole('combobox');
    expect(filterSelect).toBeInTheDocument();
    
    // Wait for dynamic content with proper timing
    await waitFor(() => {
      expect(screen.getByText('Dialog Title')).toBeInTheDocument();
    }, { timeout: 5000 });
  });
});

// Material-UI Select Component Testing Pattern
describe('Select Component Testing', () => {
  it('should handle Select dropdown interactions', async () => {
    const user = userEvent.setup();
    
    // Component with proper ARIA labels
    render(
      <FormControl>
        <InputLabel>Filter Annotations</InputLabel>
        <Select
          value={filterBy}
          onChange={handleFilterChange}
          label="Filter Annotations"
          inputProps={{
            'aria-label': 'Filter annotations by type'
          }}
          MenuProps={{ 
            disablePortal: true,
            container: document.body
          }}
        >
          <MenuItem value="all">All Annotations</MenuItem>
          <MenuItem value="pinned">Pinned Annotations</MenuItem>
        </Select>
      </FormControl>
    );
    
    // Find select using aria-label (most reliable)
    const filterSelect = screen.getByLabelText('Filter annotations by type');
    
    // Click to open dropdown
    await user.click(filterSelect);
    
    // Wait for dropdown options to appear
    await waitFor(() => {
      expect(screen.getByText('Pinned Annotations')).toBeInTheDocument();
    });
    
    // Click option
    await user.click(screen.getByText('Pinned Annotations'));
    
    // Verify selection worked (test actual behavior, not internal state)
    await waitFor(() => {
      expect(screen.getByText('Pinned Annotations')).toBeInTheDocument();
    });
  });
});
```

### **Dialog and Portal Testing**
```typescript
// Material-UI Dialog testing considerations
describe('Material-UI Dialog Components', () => {
  it('should handle dialog portals correctly', async () => {
    const user = userEvent.setup();
    render(<TestWrapper><DialogComponent /></TestWrapper>);
    
    // Click to open dialog
    await user.click(screen.getByText('Open Dialog'));
    
    // Wait for dialog to appear (portal content)
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeInTheDocument();
    }, { timeout: 5000 });
    
    // Dialog content may be in a different DOM tree due to portals
    // Use proper selectors and timing
    await waitFor(() => {
      expect(screen.getByLabelText('Form Field')).toBeInTheDocument();
    }, { timeout: 2000 });
  });
});
```

### **Test Structure**
```typescript
// Example test structure with Material-UI considerations
describe('InteractiveWorldMap', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    // Clean up portal containers for Material-UI components
    const existingContainers = document.querySelectorAll('[data-testid="portal-container"]');
    existingContainers.forEach(container => container.remove());
  });

  afterEach(() => {
    // Clean up portal containers after each test
    const containers = document.querySelectorAll('[data-testid="portal-container"]');
    containers.forEach(container => container.remove());
  });

  it('renders world map with countries', () => {
    // Test basic rendering
  });
  
  it('handles country click events', () => {
    // Test user interactions
  });
  
  it('updates data visualization on indicator change', () => {
    // Test data updates
  });
  
  it('is accessible via keyboard navigation', () => {
    // Test accessibility with proper ARIA selectors
  });
});
```

## 🚀 **Performance Standards**

### **Rendering Performance**
- **60fps Animations**: Smooth D3.js animations
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
- **CDN Assets**: Use CDN for large libraries

## 🔧 **Development Workflow**

### **Branch Strategy**
- **Feature Branches**: `frontend/feature-name`
- **Component Branches**: `frontend/component-name`
- **Bug Fix Branches**: `frontend/fix-issue-name`
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
- [React Documentation](https://reactjs.org/docs/)
- [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- [Material-UI Components](https://mui.com/components/)
- [D3.js Documentation](https://d3js.org/)
- [D3-Geo Documentation](https://github.com/d3/d3-geo)

### **Best Practices**
- [React Performance](https://reactjs.org/docs/optimizing-performance.html)
- [D3.js Best Practices](https://observablehq.com/@d3/learn-d3)
- [Accessibility Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [Material-UI Theming](https://mui.com/customization/theming/)

## 🎯 **Current Project Focus**

### **Global Analysis UI Development**
- **Phase 1**: Interactive world map with D3.js
- **Phase 2**: Multi-country dashboard enhancements
- **Phase 3**: Event visualization and network analysis
- **Phase 4**: Advanced UI features and export
- **Phase 5**: Mobile optimization and accessibility

### **Key Deliverables**
- **InteractiveWorldMap**: D3.js world map component
- **EconomicDataOverlay**: Data visualization on map
- **CountrySelector**: Advanced country selection
- **ChartVisualizations**: Multi-country comparison charts
- **ExportFeatures**: Map and data export functionality

### **Success Metrics**
- **Performance**: Map renders in < 2 seconds
- **Interactions**: Smooth 60fps zoom/pan
- **Accessibility**: Full keyboard navigation
- **Responsive**: Works on all device sizes
- **User Experience**: Intuitive and engaging

## 🚨 **Common Pitfalls to Avoid**

### **D3.js Integration**
- **Memory Leaks**: Always clean up event listeners
- **Performance**: Avoid re-rendering entire map on data updates
- **Responsive**: Handle window resize events properly
- **Accessibility**: Don't forget ARIA labels and keyboard navigation
- **ES Modules**: D3 modules use ES modules - configure Jest `transformIgnorePatterns`
- **Data Loading**: Use CDN loading for world atlas data instead of complex imports

### **React Performance**
- **Unnecessary Re-renders**: Use React.memo, useMemo, useCallback
- **State Management**: Avoid prop drilling, use context appropriately
- **Bundle Size**: Import only needed D3 modules
- **Error Handling**: Implement proper error boundaries
- **Context Updates**: Split state into logical groups to prevent unnecessary re-renders

### **Material-UI Testing Pitfalls**
- **Dialog Portal Issues**: Material-UI Dialogs use portals - require special test setup
- **Missing Providers**: Always include `StyledEngineProvider`, `ThemeProvider`, `LocalizationProvider`
- **Portal Cleanup**: Clean up portal containers between tests to prevent DOM pollution
- **ARIA Selector Mismatches**: Check actual component ARIA implementation before writing tests
- **Timing Issues**: Use proper `waitFor` with appropriate timeouts for dialog content
- **Test Environment**: Material-UI components behave differently in test vs production environments
- **CRITICAL: TextField Input Testing**: `user.type()` hangs with Material-UI TextField components - use `fireEvent.change()` instead
- **CRITICAL: Duplicate Drawer Elements**: AdminLayout and similar components render duplicate navigation elements (mobile + desktop drawers) causing "Found multiple elements" errors

### **UI/UX Issues**
- **Loading States**: Always show loading indicators
- **Error States**: Provide clear error messages
- **Accessibility**: Test with screen readers and keyboard navigation
- **Mobile**: Test on actual devices, not just browser dev tools
- **Material-UI Integration**: Use `Box` components as containers for D3.js SVG elements

## 📚 **Lessons Learned from Global Analysis Implementation**

### **Technical Discoveries**
1. **Jest Configuration for D3.js**
   - **Issue**: D3 modules use ES modules which Jest doesn't handle by default
   - **Solution**: Add D3 modules to `transformIgnorePatterns` in Jest config
   - **Impact**: Comprehensive test suite requires proper Jest configuration

2. **World Atlas Data Loading**
   - **Issue**: `world-atlas` package has complex import structure
   - **Solution**: Use CDN loading with `fetch()` for reliable data access
   - **Impact**: More robust data loading with proper error handling

3. **TypeScript Type Safety**
   - **Discovery**: Comprehensive type definitions are crucial for D3.js integration
   - **Solution**: Create 15+ specialized interfaces covering all use cases
   - **Impact**: Better developer experience and fewer runtime errors

4. **React Context API Performance**
   - **Discovery**: Context updates can cause unnecessary re-renders
   - **Solution**: Split state into logical groups and use `useCallback` for actions
   - **Impact**: Optimized performance with proper memoization

5. **Material-UI Integration**
   - **Discovery**: D3.js SVG elements need special handling with Material-UI
   - **Solution**: Use `Box` components as containers and proper event handling
   - **Impact**: Seamless integration with consistent design system

### **Material-UI Testing Discoveries**
1. **Dialog Portal Rendering Issues**
   - **Issue**: Material-UI Dialogs render content via portals, causing test failures
   - **Root Cause**: Dialog container opens but portal content doesn't render in test environment
   - **Solution**: Use `StyledEngineProvider`, proper theme setup, and custom render functions
   - **Impact**: Comprehensive dialog testing requires special setup and timing considerations

2. **ARIA Label Testing Patterns**
   - **Discovery**: Material-UI components use specific ARIA patterns that require proper test selectors
   - **Solution**: Use `getByLabelText()` for `aria-label` attributes, `getByRole()` for standard roles
   - **Best Practice**: Always check component's actual ARIA implementation before writing tests
   - **Impact**: More reliable and accessible test suites that match real user interactions

3. **Select Component Testing with ARIA Labels**
   - **Issue**: Material-UI Select components are difficult to test due to portal rendering and lack of proper ARIA labels
   - **Root Cause**: Select components don't automatically have accessible names, making `getByLabelText()` fail
   - **Solution**: Add `inputProps={{ 'aria-label': 'Descriptive label' }}` to Select components
   - **Testing Pattern**: Use `screen.getByLabelText('Descriptive label')` instead of `screen.getByTestId()`
   - **Portal Fix**: Add `MenuProps={{ disablePortal: true }}` to prevent dropdown options from rendering in portals
   - **Impact**: Reliable Select component testing that works consistently across test environments

4. **Test Setup Requirements**
   - **Discovery**: Material-UI components require comprehensive provider setup for testing
   - **Solution**: Include `ThemeProvider`, `LocalizationProvider`, `StyledEngineProvider`, and `CssBaseline`
   - **Best Practice**: Create reusable `TestWrapper` components for consistent test setup
   - **Impact**: Consistent test environment that matches production component behavior

5. **Portal Container Management**
   - **Issue**: Material-UI portals create DOM elements that persist between tests
   - **Solution**: Clean up portal containers in `beforeEach` and `afterEach` hooks
   - **Best Practice**: Use `data-testid` attributes for reliable portal container identification
   - **Impact**: Isolated tests without DOM pollution from previous test runs

6. **Timing and Async Testing**
   - **Discovery**: Material-UI dialogs have complex render timing that requires proper `waitFor` usage
   - **Solution**: Use appropriate timeouts (5000ms for dialogs, 2000ms for content) and proper async/await patterns
   - **Best Practice**: Always wait for dialog titles before checking dialog content
   - **Impact**: Reliable test execution that accounts for Material-UI's render cycles

7. **CRITICAL: TextField Input Testing with user.type() vs fireEvent.change()**
   - **Issue**: `user.type()` from `@testing-library/user-event` hangs indefinitely with Material-UI TextField components
   - **Root Cause**: Material-UI's controlled TextField components don't properly handle `user.type()` simulation in Jest/jsdom environment
   - **Solution**: Use `fireEvent.change()` instead of `user.type()` for Material-UI TextField components
   - **Testing Pattern**: 
     ```typescript
     // ❌ BAD: Will hang indefinitely
     await user.type(titleField, 'New Annotation');
     
     // ✅ GOOD: Works reliably
     fireEvent.change(titleField, { target: { value: 'New Annotation' } });
     ```
   - **Impact**: This single change can fix multiple test failures and prevent test suite hanging issues
   - **Scope**: Applies to all Material-UI TextField, TextareaAutosize, and similar controlled input components

8. **CRITICAL: Duplicate Drawer Elements in AdminLayout Components**
   - **Issue**: AdminLayout and similar components render duplicate navigation elements (mobile + desktop drawers) causing "Found multiple elements" errors in tests
   - **Root Cause**: Material-UI responsive drawer pattern renders both mobile (temporary) and desktop (permanent) drawers simultaneously, creating duplicate DOM elements
   - **Symptoms**: Tests fail with "Found multiple elements with the text: [Navigation Item]" errors and timeout at exactly 30 seconds
   - **Solution**: Use `within()` to target specific drawer containers instead of `getAllByText()[0]` hacks
   - **Testing Pattern**: 
     ```typescript
     // ❌ BAD: Causes "Found multiple elements" errors
     expect(screen.getByText("Dashboard")).toBeInTheDocument();
     expect(screen.getByText("2 security event(s)")).toBeInTheDocument();
     
     // ✅ GOOD: Target specific drawer container
     const desktopDrawer = screen.getByRole("navigation");
     expect(within(desktopDrawer).getByText("Dashboard")).toBeInTheDocument();
     expect(within(desktopDrawer).getByText("2 security event(s)")).toBeInTheDocument();
     ```
   - **Scope**: Applies to all components using Material-UI responsive drawer patterns (AdminLayout, navigation components)
   - **Impact**: Prevents systematic test timeouts and "Found multiple elements" errors across entire test suites

9. **CRITICAL: Systematic Duplicate Element Detection and Fixing**
   - **Issue**: Duplicate element issues are often systematic across entire test suites, not isolated incidents
   - **Root Cause**: Material-UI responsive patterns, portal rendering, and component composition create predictable duplicate element scenarios
   - **Detection Strategy**: Search for patterns like `getAllByText()[0]`, `getAllByText()[1]`, or "Found multiple elements" errors
   - **Systematic Fix Approach**:
     ```bash
     # 1. Search for fragile patterns
     grep -r "getAllByText.*\[0\]" src/
     grep -r "Found multiple elements" src/
     
     # 2. Identify common container patterns
     grep -r "getByRole.*navigation" src/
     grep -r "getByRole.*dialog" src/
     ```
   - **Fix Pattern**: Replace all instances with proper `within()` strategy
     ```typescript
     // ❌ BAD: Fragile hack that breaks easily
     expect(screen.getAllByText("Dashboard")[0]).toBeInTheDocument();
     expect(screen.getAllByText("User Management")[0]).toBeInTheDocument();
     
     // ✅ GOOD: Systematic, reliable approach
     const desktopDrawer = screen.getByRole("navigation");
     expect(within(desktopDrawer).getByText("Dashboard")).toBeInTheDocument();
     expect(within(desktopDrawer).getByText("User Management")).toBeInTheDocument();
     ```
   - **Common Duplicate Element Scenarios**:
     - **Responsive Drawers**: Mobile + desktop drawers render simultaneously
     - **Portal Dialogs**: Multiple dialog instances in DOM
     - **Conditional Rendering**: Components render in multiple contexts
     - **Navigation Menus**: Mobile + desktop navigation elements
   - **Prevention Strategy**: Always use semantic selectors (`getByRole`, `getByLabelText`) and `within()` for scoped queries
   - **Impact**: Fixing systematic duplicate element issues can resolve 50%+ of test failures in complex Material-UI applications

### **Testing Strategy Insights**
1. **Mock Strategy**: Comprehensive D3.js mocking required for Jest compatibility
2. **Test Coverage**: 100+ test cases across components, hooks, and context
3. **Integration Testing**: E2E tests needed for full D3.js functionality validation
4. **Performance Testing**: Large dataset testing crucial for production readiness

### **Architecture Decisions**
1. **Custom Hooks**: Separated D3.js logic into reusable hooks for better testability
2. **Context API**: Centralized state management for complex map interactions
3. **Component Composition**: Modular design allows for easy feature additions
4. **Type Safety**: Comprehensive TypeScript coverage prevents runtime errors

### **Best Practices Established**
1. **Component Structure**: Always separate D3.js logic into custom hooks
2. **State Management**: Use Context API for complex state, local state for simple UI
3. **Testing**: Create comprehensive test suites with proper mocking
4. **Performance**: Implement proper memoization and cleanup
5. **Accessibility**: Build accessibility features from the start, not as an afterthought

## 🧪 **Testing Standards**

### **Accessibility and Testing Attribute Guidelines**

For comprehensive guidance on choosing between `aria-label`, `role`, and `data-testid` attributes, see our detailed [Accessibility Testing Guidelines](../docs/technical/accessibility-testing-guidelines.md). This covers when to use each attribute, common anti-patterns to avoid, and best practices for Material-UI component testing.

### **Recommended Accessibility Testing Tools**

#### **Static Analysis: eslint-plugin-jsx-a11y**
- **Purpose**: Catches accessibility issues during development
- **Installation**: `npm install --save-dev eslint-plugin-jsx-a11y`
- **Coverage**: 50+ accessibility rules for JSX/React components
- **Integration**: Add to ESLint config for automatic linting

#### **Runtime Testing: axe-core-react**
- **Purpose**: Automated WCAG compliance testing
- **Installation**: `npm install --save-dev @axe-core/react`
- **Usage**: Integrates with Jest and React Testing Library
- **Benefits**: Comprehensive accessibility violation detection

#### **Testing Strategy**
```typescript
// Example accessibility test with axe-core
import { axe, toHaveNoViolations } from 'jest-axe';

expect.extend(toHaveNoViolations);

test('should not have accessibility violations', async () => {
  const { container } = render(<MyComponent />);
  const results = await axe(container);
  expect(results).toHaveNoViolations();
});
```

### **Core Testing Philosophy**
1. **Test Behavior, Not Implementation**: Focus on what users can see and do, not internal component structure
2. **Write Tests That Fail for the Right Reasons**: Tests should catch real bugs, not implementation changes
3. **Make Components Testable by Design**: Build testability into components from the start
4. **Use the Testing Pyramid**: Unit tests for logic, integration tests for interactions, E2E tests for user flows
5. **Test Accessibility**: Ensure components work for all users, including those using assistive technologies

### **Key Testing Principles**

#### **ARIA Labels Are Essential for Both Accessibility and Testing**
- **Principle**: Every interactive element should have proper ARIA labels
- **Why**: ARIA labels provide the most reliable way to find elements in tests
- **Implementation**: Add `inputProps={{ 'aria-label': 'Descriptive label' }}` to Material-UI Select components
- **Testing**: Use `screen.getByLabelText('Descriptive label')` instead of fragile selectors
- **Benefit**: Tests become more robust and components become more accessible

#### **CRITICAL: user.type() vs fireEvent.change() for Material-UI TextField Components**
- **Discovery**: `user.type()` hangs indefinitely with Material-UI TextField components in Jest/jsdom
- **Root Cause**: Material-UI's controlled components don't properly handle `user.type()` simulation
- **Solution**: Always use `fireEvent.change()` for Material-UI TextField components
- **Pattern**: `fireEvent.change(field, { target: { value: 'text' } })` instead of `await user.type(field, 'text')`
- **Impact**: This can fix 50%+ of Material-UI test failures and prevent test suite hanging

#### **Material-UI Components Need Special Test Setup**
- **Portal Issues**: Use `MenuProps={{ disablePortal: true }}` for Select components
- **Provider Setup**: Always include all required Material-UI providers in test environment
- **Timing**: Use appropriate `waitFor` timeouts for portal content rendering
- **Cleanup**: Clean up portal containers between tests to prevent DOM pollution

### **Component Testability Design Patterns**

#### **Separation of Concerns**
```typescript
// ❌ BAD: Mixed concerns make testing difficult
const ComplexComponent = () => {
  const [data, setData] = useState([]);
  const [loading, setLoading] = useState(false);
  
  // Business logic mixed with UI
  const processData = (rawData) => {
    return rawData.filter(item => item.active)
                  .map(item => ({ ...item, processed: true }));
  };
  
  // API calls mixed with UI
  useEffect(() => {
    fetchData().then(setData);
  }, []);
  
  return <div>{/* Complex UI */}</div>;
};

// ✅ GOOD: Separated concerns enable focused testing
const useDataProcessing = (rawData) => {
  return useMemo(() => 
    rawData.filter(item => item.active)
           .map(item => ({ ...item, processed: true })), 
    [rawData]
  );
};

const useDataFetching = () => {
  const [data, setData] = useState([]);
  const [loading, setLoading] = useState(false);
  
  useEffect(() => {
    setLoading(true);
    fetchData().then(setData).finally(() => setLoading(false));
  }, []);
  
  return { data, loading };
};

const SimpleComponent = () => {
  const { data: rawData, loading } = useDataFetching();
  const processedData = useDataProcessing(rawData);
  
  return <div>{/* Simple UI */}</div>;
};
```

#### **Pure Function Extraction**
```typescript
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

#### **3. Dependency Injection for Testability**
```typescript
// ❌ BAD: Hard-coded dependencies make testing difficult
const UserProfile = () => {
  const [user, setUser] = useState(null);
  
  useEffect(() => {
    // Hard to mock this API call
    fetch('/api/user').then(res => res.json()).then(setUser);
  }, []);
  
  return <div>{user?.name}</div>;
};

// ✅ GOOD: Injected dependencies enable easy mocking
interface UserProfileProps {
  userService?: UserService;
}

const UserProfile = ({ userService = defaultUserService }: UserProfileProps) => {
  const [user, setUser] = useState(null);
  
  useEffect(() => {
    userService.getCurrentUser().then(setUser);
  }, [userService]);
  
  return <div>{user?.name}</div>;
};

// Easy to test with mock service
const mockUserService = {
  getCurrentUser: jest.fn().mockResolvedValue({ name: 'Test User' })
};

test('should display user name', async () => {
  render(<UserProfile userService={mockUserService} />);
  await waitFor(() => {
    expect(screen.getByText('Test User')).toBeInTheDocument();
  });
});
```

### **Robust Test Selector Strategies**

#### **1. Accessibility-First Selectors**
```typescript
// ❌ BAD: Fragile selectors that break with UI changes
test('should submit form', () => {
  const submitButton = screen.getByText('Submit'); // Breaks if text changes
  fireEvent.click(submitButton);
});

// ✅ GOOD: Semantic selectors that match user intent
test('should submit form', () => {
  const submitButton = screen.getByRole('button', { name: /submit/i });
  fireEvent.click(submitButton);
});

// Even better: Use data-testid for complex interactions
test('should submit form with validation', () => {
  const submitButton = screen.getByTestId('submit-form-button');
  fireEvent.click(submitButton);
  expect(screen.getByRole('alert')).toBeInTheDocument();
});
```

#### **2. Progressive Selector Fallbacks**
```typescript
// Robust selector strategy with fallbacks
const findFormField = (fieldName: string) => {
  // 1. Try semantic selector first (most accessible)
  try {
    return screen.getByLabelText(fieldName);
  } catch {
    // 2. Fall back to placeholder
    try {
      return screen.getByPlaceholderText(fieldName);
    } catch {
      // 3. Fall back to data-testid
      return screen.getByTestId(`${fieldName.toLowerCase()}-input`);
    }
  }
};

test('should fill form field', () => {
  const nameField = findFormField('Name');
  fireEvent.change(nameField, { target: { value: 'John Doe' } });
  expect(nameField).toHaveValue('John Doe');
});
```

#### **3. Portal-Aware Testing**
```typescript
// Material-UI Dialogs and other portal components
test('should handle dialog interactions', async () => {
  const user = userEvent.setup();
  
  // Open dialog
  await user.click(screen.getByRole('button', { name: 'Open Dialog' }));
  
  // Wait for portal content to render
  await waitFor(() => {
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  }, { timeout: 5000 });
  
  // Interact with dialog content (may be in different DOM tree)
  await waitFor(() => {
    const input = screen.getByLabelText('Dialog Input');
    expect(input).toBeInTheDocument();
  }, { timeout: 2000 });
  
  await user.type(input, 'Test Value');
  expect(input).toHaveValue('Test Value');
});
```

### **Test Organization Patterns**

#### **1. Test Structure by Complexity**
```typescript
describe('ComplexComponent', () => {
  // Setup and teardown
  beforeEach(() => {
    jest.clearAllMocks();
    setupTestEnvironment();
  });
  
  afterEach(() => {
    cleanupTestEnvironment();
  });
  
  // Group tests by functionality
  describe('Basic Rendering', () => {
    it('should render without crashing', () => {
      render(<ComplexComponent />);
      expect(screen.getByRole('main')).toBeInTheDocument();
    });
    
    it('should display loading state', () => {
      render(<ComplexComponent loading={true} />);
      expect(screen.getByText(/loading/i)).toBeInTheDocument();
    });
  });
  
  describe('User Interactions', () => {
    it('should handle button clicks', async () => {
      const user = userEvent.setup();
      render(<ComplexComponent />);
      
      await user.click(screen.getByRole('button', { name: /submit/i }));
      expect(mockOnSubmit).toHaveBeenCalled();
    });
  });
  
  describe('Error Handling', () => {
    it('should display error messages', () => {
      render(<ComplexComponent error="Test error" />);
      expect(screen.getByRole('alert')).toHaveTextContent('Test error');
    });
  });
});
```

#### **2. Mock Strategy Patterns**
```typescript
// Centralized mock setup
const createMockProps = (overrides = {}) => ({
  data: mockData,
  onAction: jest.fn(),
  loading: false,
  error: null,
  ...overrides
});

// Reusable mock implementations
const mockApiService = {
  fetchData: jest.fn(),
  updateData: jest.fn(),
  deleteData: jest.fn(),
};

// Reset mocks between tests
beforeEach(() => {
  Object.values(mockApiService).forEach(mock => mock.mockClear());
});
```

### **Component Design for Testability**

#### **1. Prop Interface Design**
```typescript
// ✅ GOOD: Clear, testable prop interface
interface ChartProps {
  data: ChartData[];
  onDataPointClick?: (point: DataPoint) => void;
  loading?: boolean;
  error?: string | null;
  config?: Partial<ChartConfig>;
  'data-testid'?: string; // Always include for testing
}

// ❌ BAD: Unclear or hard-to-test props
interface BadChartProps {
  data: any; // Too generic
  onClick: Function; // Too generic
  style?: React.CSSProperties; // Hard to test styling
}
```

#### **2. Event Handler Patterns**
```typescript
// ✅ GOOD: Testable event handlers
const ChartComponent = ({ data, onDataPointClick }: ChartProps) => {
  const handlePointClick = useCallback((event: MouseEvent, point: DataPoint) => {
    if (onDataPointClick) {
      onDataPointClick(point);
    }
  }, [onDataPointClick]);
  
  return (
    <Chart 
      data={data} 
      onPointClick={handlePointClick}
      data-testid="interactive-chart"
    />
  );
};

// Easy to test
test('should call onDataPointClick when point is clicked', async () => {
  const mockOnClick = jest.fn();
  render(<ChartComponent data={mockData} onDataPointClick={mockOnClick} />);
  
  await user.click(screen.getByTestId('interactive-chart'));
  expect(mockOnClick).toHaveBeenCalledWith(expect.objectContaining({
    id: expect.any(String),
    value: expect.any(Number)
  }));
});
```

### **Key Testing Principles from Real-World Debugging**

#### **1. ARIA Labels Are Essential for Both Accessibility and Testing**
- **Principle**: Every interactive element should have proper ARIA labels
- **Why**: ARIA labels provide the most reliable way to find elements in tests
- **Implementation**: Add `inputProps={{ 'aria-label': 'Descriptive label' }}` to Material-UI Select components
- **Testing**: Use `screen.getByLabelText('Descriptive label')` instead of fragile selectors
- **Benefit**: Tests become more robust and components become more accessible

#### **2. Debug Systematically, Don't Give Up**
- **Approach**: When tests fail, add debugging code to understand what's actually happening
- **Method**: Use `console.log()` to inspect DOM structure, element properties, and timing
- **Pattern**: Start with the most likely cause, then systematically eliminate possibilities
- **Example**: Log all elements, check aria attributes, verify portal rendering, test different selectors

#### **3. Test Real Behavior, Not Internal Implementation**
- **Anti-Pattern**: Testing internal state values like `toHaveValue('all')`
- **Better Approach**: Test the actual user-visible behavior (filtering results, UI changes)
- **Example**: Instead of checking if a select has value 'pinned', check if pinned items are visible
- **Benefit**: Tests remain stable when internal implementation changes

#### **4. Material-UI Components Need Special Test Setup**
- **Portal Issues**: Use `MenuProps={{ disablePortal: true }}` for Select components
- **Provider Setup**: Always include all required Material-UI providers in test environment
- **Timing**: Use appropriate `waitFor` timeouts for portal content rendering
- **Cleanup**: Clean up portal containers between tests to prevent DOM pollution

#### **5. CRITICAL: user.type() vs fireEvent.change() for Material-UI TextField Components**
- **Discovery**: `user.type()` hangs indefinitely with Material-UI TextField components in Jest/jsdom
- **Root Cause**: Material-UI's controlled components don't properly handle `user.type()` simulation
- **Solution**: Always use `fireEvent.change()` for Material-UI TextField components
- **Pattern**: `fireEvent.change(field, { target: { value: 'text' } })` instead of `await user.type(field, 'text')`
- **Impact**: This can fix 50%+ of Material-UI test failures and prevent test suite hanging

#### **6. CRITICAL: Systematic Duplicate Element Debugging Methodology**
- **Problem Recognition**: When you see "Found multiple elements" errors, it's usually systematic, not isolated
- **Detection Strategy**: Search for patterns like `getAllByText()[0]` or `getAllByText()[1]` in test files
- **Root Cause Analysis**: Material-UI responsive patterns create predictable duplicate scenarios
- **Systematic Fix Process**:
  1. **Identify Pattern**: Search for `getAllByText.*\[0\]` across all test files
  2. **Find Container**: Look for semantic containers like `getByRole("navigation")` or `getByRole("dialog")`
  3. **Apply within()**: Replace all instances with `within(container).getByText(...)`
  4. **Verify Fix**: Run tests to ensure no more "Found multiple elements" errors
- **Prevention**: Always use semantic selectors and `within()` for scoped queries from the start
- **Impact**: This systematic approach can fix 50%+ of test failures in complex Material-UI applications

#### **7. CRITICAL: Debugging Commands and Patterns for Duplicate Elements**
- **Detection Commands**:
  ```bash
  # Find all fragile getAllByText patterns
  grep -r "getAllByText.*\[0\]" src/
  grep -r "getAllByText.*\[1\]" src/
  
  # Find "Found multiple elements" error patterns
  grep -r "Found multiple elements" src/
  
  # Find common container patterns
  grep -r "getByRole.*navigation" src/
  grep -r "getByRole.*dialog" src/
  ```
- **Debugging Strategy**:
  ```typescript
  // Add debugging to understand DOM structure
  console.log('All elements with text "Dashboard":', screen.getAllByText("Dashboard"));
  console.log('Navigation role elements:', screen.getAllByRole("navigation"));
  console.log('Desktop drawer:', screen.getByRole("navigation"));
  ```
- **Common Fix Patterns**:
  ```typescript
  // ❌ BAD: Fragile array indexing
  expect(screen.getAllByText("Dashboard")[0]).toBeInTheDocument();
  
  // ✅ GOOD: Semantic container targeting
  const desktopDrawer = screen.getByRole("navigation");
  expect(within(desktopDrawer).getByText("Dashboard")).toBeInTheDocument();
  ```
- **Verification Commands**:
  ```bash
  # Run specific failing tests to verify fixes
  npm test -- --testNamePattern="displays user information correctly"
  npm test -- --testNamePattern="formats session time correctly"
  ```
- **Impact**: This systematic debugging approach can identify and fix 8+ duplicate element issues in a single pass

### **React Query + Suspense Best Practices**

#### **Modern Data Fetching Patterns**
```typescript
// ❌ BAD: Manual loading state management
const Component = () => {
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  
  useEffect(() => {
    setLoading(true);
    fetchData()
      .then(setData)
      .catch(setError)
      .finally(() => setLoading(false));
  }, []);
  
  if (loading) return <CircularProgress />;
  if (error) return <Alert severity="error">{error}</Alert>;
  return <div>{data?.content}</div>;
};

// ✅ GOOD: React Query + Suspense for declarative loading
const useComponentData = () => {
  return useQuery({
    queryKey: ['component-data'],
    queryFn: fetchData,
    suspense: true, // Enable Suspense integration
    staleTime: 5 * 60 * 1000, // 5 minutes
  });
};

const ComponentContent = () => {
  const { data } = useComponentData(); // No loading check needed!
  return <div>{data.content}</div>;
};

const Component = () => (
  <Suspense fallback={<CircularProgress />}>
    <ErrorBoundary fallback={<Alert severity="error">Error loading data</Alert>}>
      <ComponentContent />
    </ErrorBoundary>
  </Suspense>
);
```

#### **Benefits of React Query + Suspense**
- ✅ **~50-80 lines less code per component** - No manual loading/error states
- ✅ **Automatic caching** - Data persists across component unmounts
- ✅ **Automatic retries** - Built-in error recovery
- ✅ **Declarative loading states** - Suspense boundaries handle loading UI
- ✅ **Better code organization** - Separate data fetching from presentation
- ✅ **Easier testing** - Mock data at the hook level

#### **Multiple Query Coordination**
```typescript
// ✅ GOOD: Multiple queries with Suspense
const useFinancialData = (companyId: string) => {
  const statements = useQuery({
    queryKey: ['statements', companyId],
    queryFn: () => fetchStatements(companyId),
    suspense: true,
  });
  
  const ratios = useQuery({
    queryKey: ['ratios', companyId],
    queryFn: () => fetchRatios(companyId),
    suspense: true,
  });
  
  const annotations = useQuery({
    queryKey: ['annotations', companyId],
    queryFn: () => fetchAnnotations(companyId),
    suspense: true,
  });
  
  return { statements: statements.data, ratios: ratios.data, annotations: annotations.data };
};

// Component automatically suspends until ALL queries resolve
const FinancialDashboard = ({ companyId }: Props) => {
  const { statements, ratios, annotations } = useFinancialData(companyId);
  // All data guaranteed to be present here!
  return <div>...</div>;
};
```

#### **Infinite Cache for Static Data**
```typescript
// For data that never changes (world atlas, static configs)
const useWorldAtlasData = () => {
  return useQuery({
    queryKey: ['world-atlas'],
    queryFn: async () => {
      const response = await fetch('https://cdn.jsdelivr.net/npm/world-atlas@2/countries-110m.json');
      return response.json();
    },
    suspense: true,
    staleTime: Infinity, // Never refetch
    cacheTime: Infinity, // Never garbage collect
  });
};
```

### **Testing Library Query Priority - Critical Lessons**

#### **The Query Priority Hierarchy**
Testing Library has a recommended query priority. **Always follow this order:**

1. **`getByRole`** - Most accessible and robust
2. **`getByLabelText`** - For form fields
3. **`getByPlaceholderText`** - For inputs without labels
4. **`getByText`** - For non-interactive elements
5. **`getByDisplayValue`** - For form inputs with values
6. **`getByAltText`** - For images
7. **`getByTitle`** - Last resort
8. **`getByTestId`** - ONLY when semantic queries are impossible

#### **CRITICAL: Multiple Elements vs Single Elements**

**The most common test failure pattern:**
```typescript
// ❌ BAD: Will throw "Found multiple elements" if there are duplicates
const element = screen.getByText('Financial Alerts'); // THROWS if >1 found
const button = screen.findByRole('button', { name: 'Submit' }); // THROWS if >1 found

// ✅ GOOD: Use getAllBy* to check for presence
expect(screen.getAllByText('Financial Alerts').length).toBeGreaterThan(0);
expect(screen.getAllByRole('button', { name: 'Submit' }).length).toBeGreaterThan(0);

// ✅ GOOD: Use queryBy* for optional elements
const optionalButton = screen.queryByRole('button', { name: 'Refresh' });
if (optionalButton) {
  fireEvent.click(optionalButton);
}

// ✅ GOOD: Use queryAllBy* for optional multiple elements
const refreshButtons = screen.queryAllByRole('button', { name: /refresh/i });
if (refreshButtons.length > 0) {
  fireEvent.click(refreshButtons[0]);
}
```

#### **Query Variants: When to Use Each**

| Variant | Returns | Throws if not found? | Waits? | Use case |
|---------|---------|---------------------|--------|----------|
| `getBy*` | Single element | ✅ Yes | ❌ No | Required single element |
| `getAllBy*` | Array | ✅ Yes (if none) | ❌ No | Required multiple elements |
| `queryBy*` | Single element or null | ❌ No | ❌ No | Optional single element |
| `queryAllBy*` | Array (empty if none) | ❌ No | ❌ No | Optional multiple elements |
| `findBy*` | Promise<element> | ✅ Yes (after timeout) | ✅ Yes | Async single element |
| `findAllBy*` | Promise<array> | ✅ Yes (after timeout) | ✅ Yes | Async multiple elements |

#### **Real-World Test Fixes**

**Problem: "Found multiple elements" error**
```typescript
// ❌ BAD: Fails with "Found multiple elements with the text: Data Sources"
test('should render page title', () => {
  render(<DataSources />);
  expect(screen.getByText('Data Sources')).toBeInTheDocument();
});

// ✅ GOOD: Handle multiple elements correctly
test('should render page title', () => {
  render(<DataSources />);
  expect(screen.getAllByText('Data Sources').length).toBeGreaterThan(0);
});

// ✅ EVEN BETTER: Use semantic query
test('should render page title', () => {
  render(<DataSources />);
  expect(screen.getByRole('heading', { name: 'Data Sources', level: 1 })).toBeInTheDocument();
});
```

**Problem: Test timeout waiting for element**
```typescript
// ❌ BAD: waitFor with getAllBy* can timeout if element doesn't exist
await waitFor(() => {
  expect(screen.getAllByText('Financial Alerts').length).toBeGreaterThan(0);
});

// ✅ GOOD: Use findBy* for async elements
await screen.findByRole('heading', { name: /financial alerts/i });

// ✅ GOOD: Use findBy* with semantic selector
await screen.findByRole('heading', { level: 1, name: /data sources/i });
```

**Problem: Multiple assertions in waitFor**
```typescript
// ❌ BAD: ESLint error - no-wait-for-multiple-assertions
await waitFor(() => {
  const headings = screen.getAllByRole('heading', { level: 1 });
  expect(headings.length).toBeGreaterThan(0);
  expect(headings[0]).toHaveTextContent('Data Sources');
});

// ✅ GOOD: Use findBy* then make assertions
await screen.findByRole('heading', { level: 1, name: /data sources/i });

const headings = screen.getAllByRole('heading', { level: 1 });
expect(headings.length).toBeGreaterThan(0);
expect(headings[0]).toHaveTextContent('Data Sources');
```

#### **Scoping Queries with within()**
```typescript
// When you have duplicate elements, scope to a container
import { within } from '@testing-library/react';

// ❌ BAD: Ambiguous - which drawer?
expect(screen.getByText('Dashboard')).toBeInTheDocument(); // Multiple drawers!

// ✅ GOOD: Scoped to specific container
const desktopDrawer = screen.getByRole('navigation');
expect(within(desktopDrawer).getByText('Dashboard')).toBeInTheDocument();

// ✅ GOOD: Scoped to dialog
const dialog = screen.getByRole('dialog');
const titleInput = within(dialog).getByLabelText('Title');
fireEvent.change(titleInput, { target: { value: 'New Title' } });
```

### **MSW (Mock Service Worker) Best Practices**

#### **JSON-Backed Mocks vs In-Code Mocks**
```typescript
// ❌ BAD: In-code mocks are hard to maintain and inconsistent
server.use(
  graphql.query('GetFinancialAlerts', (req, res, ctx) => {
    return res(ctx.data({
      financialAlerts: [
        { id: '1', title: 'Alert 1', severity: 'high', ... },
        { id: '2', title: 'Alert 2', severity: 'medium', ... },
        // 50+ lines of mock data...
      ]
    }));
  })
);

// ✅ GOOD: JSON files are centralized and reusable
// frontend/src/__tests__/mocks/graphql/get_financial_alerts/success.json
{
  "data": {
    "financialAlerts": [
      { "id": "1", "title": "Alert 1", "severity": "high", ... }
    ]
  }
}

// frontend/src/__tests__/mocks/server.ts
import getFinancialAlertsSuccess from './graphql/get_financial_alerts/success.json';

server.use(
  graphql.query('GetFinancialAlerts', (req, res, ctx) => {
    return res(ctx.json(getFinancialAlertsSuccess));
  })
);
```

#### **Benefits of JSON-Backed Mocks**
- ✅ **Single source of truth** - All tests use same mock data
- ✅ **Easier to update** - Change data in one place
- ✅ **Schema validation** - Catch schema mismatches early
- ✅ **Reusable across tests** - Import same mocks in multiple test files
- ✅ **Version control** - Track changes to mock data over time

#### **GraphQL Schema Alignment**
```typescript
// CRITICAL: Backend schema is the source of truth!
// Always use snake_case to match backend, not camelCase

// ❌ BAD: Frontend using camelCase when backend uses snake_case
interface Annotation {
  annotationId: string;      // Backend uses annotation_id
  chartId: string;            // Backend uses chart_id
  createdAt: string;          // Backend uses created_at
  isPinned: boolean;          // Backend uses is_pinned
}

// ✅ GOOD: Match backend schema exactly
interface Annotation {
  annotation_id: string;
  chart_id: string;
  created_at: string;
  is_pinned: boolean;
}

// GraphQL query must also match
const GET_ANNOTATIONS = gql`
  query GetAnnotations($chartId: ID!) {
    annotationsForChart(chartId: $chartId) {
      annotation_id
      chart_id
      created_at
      is_pinned
    }
  }
`;
```

#### **MSW Handler Reset Best Practices**
```typescript
// ❌ BAD: Handlers persist between tests causing conflicts
describe('Component Tests', () => {
  it('test 1', () => {
    server.use(
      graphql.query('GetData', (req, res, ctx) => res(ctx.data({ custom: 'data' })))
    );
    // Handler persists to next test!
  });
  
  it('test 2', () => {
    // Still using handler from test 1!
  });
});

// ✅ GOOD: Reset handlers between tests
describe('Component Tests', () => {
  beforeEach(() => {
    server.resetHandlers(); // Clear all custom handlers
  });
  
  it('test 1', () => {
    server.use(
      graphql.query('GetData', (req, res, ctx) => res(ctx.data({ custom: 'data' })))
    );
  });
  
  it('test 2', () => {
    // Clean slate - back to default handlers
  });
});
```

### **Common Test Failure Patterns and Solutions**

#### **Pattern 1: Timeout Waiting for Element**
**Symptom:** `Test timed out in 10000ms`

**Root Cause:** Using wrong query type or waiting for element that doesn't exist

**Solution:**
```typescript
// ❌ BAD: findAllByRole waits until timeout if no elements
const buttons = await screen.findAllByRole('button', { name: /refresh/i }); // Waits 10s!

// ✅ GOOD: queryAllByRole returns immediately
const buttons = screen.queryAllByRole('button', { name: /refresh/i });
if (buttons.length > 0) {
  fireEvent.click(buttons[0]);
}
```

#### **Pattern 2: Component Stuck in Loading State**
**Symptom:** Test finds loading spinner but never finds actual content

**Root Cause:** Component's data fetching hook not setting loading to false

**Solution:**
```typescript
// ❌ BAD: Forgetting to set loading: false
const loadData = async () => {
  setState({ ...state, loading: true });
  const data = await fetchData();
  setState({ ...state, data }); // loading still true!
};

// ✅ GOOD: Always set loading: false
const loadData = async () => {
  setState({ ...state, loading: true });
  const data = await fetchData();
  setState({ data, loading: false }); // Explicitly set loading
};

// ✅ BETTER: Use React Query and eliminate manual loading state
const { data, isLoading } = useQuery({
  queryKey: ['data'],
  queryFn: fetchData,
});
```

#### **Pattern 3: Infinite Loop in useEffect**
**Symptom:** Tests hang, component re-renders infinitely, MSW shows repeated requests

**Root Cause:** Dependency array includes mutable value that triggers re-fetch

**Solution:**
```typescript
// ❌ BAD: loadCollaborators in dependency array triggers infinite loop
useEffect(() => {
  loadCollaborators(chartId);
}, [chartId, loadCollaborators]); // loadCollaborators changes every render!

// ✅ GOOD: Remove function from dependencies
useEffect(() => {
  loadCollaborators(chartId);
}, [chartId]); // Only re-run when chartId changes

// ✅ BETTER: Use useCallback to stabilize function reference
const loadCollaborators = useCallback(async (id: string) => {
  // fetch logic
}, []); // Empty deps means function reference never changes

useEffect(() => {
  loadCollaborators(chartId);
}, [chartId, loadCollaborators]); // Now safe!
```

### **Testing Anti-Patterns to Avoid**

#### **Testing Implementation Details**
```typescript
// ❌ BAD: Testing internal state or methods
test('should update internal state', () => {
  const component = render(<MyComponent />);
  const instance = component.getInstance();
  expect(instance.state.isOpen).toBe(false);
  
  instance.toggle(); // Testing internal method
  expect(instance.state.isOpen).toBe(true);
});

// ✅ GOOD: Testing user-visible behavior
test('should open dialog when button is clicked', async () => {
  const user = userEvent.setup();
  render(<MyComponent />);
  
  await user.click(screen.getByRole('button', { name: /open/i }));
  expect(screen.getByRole('dialog')).toBeInTheDocument();
});
```

#### **Over-Mocking**
```typescript
// ❌ BAD: Mocking everything makes tests brittle
jest.mock('react', () => ({
  ...jest.requireActual('react'),
  useState: jest.fn(),
  useEffect: jest.fn(),
}));

// ✅ GOOD: Mock only what's necessary
jest.mock('../api/userService', () => ({
  getCurrentUser: jest.fn(),
}));
```

#### **Ignoring Accessibility**
```typescript
// ❌ BAD: Not testing accessibility
test('should display user name', () => {
  render(<UserProfile user={{ name: 'John' }} />);
  expect(screen.getByText('John')).toBeInTheDocument();
});

// ✅ GOOD: Testing accessibility
test('should display user name with proper semantics', () => {
  render(<UserProfile user={{ name: 'John' }} />);
  
  const nameElement = screen.getByText('John');
  expect(nameElement).toBeInTheDocument();
  expect(nameElement).toHaveAttribute('aria-label', 'User name: John');
});
```

This persona provides comprehensive guidance for frontend development in the EconGraph project, with a focus on creating world-class data visualization interfaces.
