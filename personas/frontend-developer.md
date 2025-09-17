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

### **Test Structure**
```typescript
// Example test structure
describe('InteractiveWorldMap', () => {
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
    // Test accessibility
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

### **React Performance**
- **Unnecessary Re-renders**: Use React.memo, useMemo, useCallback
- **State Management**: Avoid prop drilling, use context appropriately
- **Bundle Size**: Import only needed D3 modules
- **Error Handling**: Implement proper error boundaries

### **UI/UX Issues**
- **Loading States**: Always show loading indicators
- **Error States**: Provide clear error messages
- **Accessibility**: Test with screen readers and keyboard navigation
- **Mobile**: Test on actual devices, not just browser dev tools

This persona provides comprehensive guidance for frontend development in the EconGraph project, with a focus on creating world-class data visualization interfaces.
