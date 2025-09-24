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
