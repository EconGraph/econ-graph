# Storybook Testing Workflow

This document outlines the Storybook testing setup and workflow for the frontend application.

## Overview

Storybook testing enables us to test UI components in isolation using the same test infrastructure and mocks as our regular tests. This provides visual testing capabilities and component-level testing that complements our existing test suite.

## Architecture

### Core Components

1. **Storybook Configuration** (`.storybook/`)
   - `main.tsx` - Main Storybook configuration with Vite integration
   - `preview.tsx` - Global decorators and parameters
   - `test-runner.config.ts` - Test runner configuration

2. **Story Files** (`src/**/*.stories.tsx`)
   - Component stories with different scenarios
   - Integration with existing MSW mocks
   - Error boundary and provider setup

3. **Test Infrastructure**
   - MSW (Mock Service Worker) integration
   - React Query provider setup
   - Material-UI theme and localization providers

## Key Features

### MSW Integration
- **Core Philosophy**: MSW is central to our testing approach
- **Mock Scenarios**: Success, error, loading states
- **GraphQL Responses**: Reuses existing test data
- **Browser Compatibility**: Handles Node.js imports in browser environment

### Provider Setup
- **QueryClient**: Isolated per story for test isolation
- **ErrorBoundary**: Catches and displays component errors
- **ThemeProvider**: Material-UI theming
- **LocalizationProvider**: Date picker localization

### Story Organization
- **Component Stories**: One story file per component
- **Scenario Coverage**: Different user types and states
- **Viewport Testing**: Mobile, tablet, desktop
- **Error States**: Loading and error scenarios

## Usage

### Viewing Stories

1. **Start Storybook Development Server**
   ```bash
   cd frontend
   npm run storybook
   ```

2. **Open Browser**
   Navigate to `http://localhost:6006` to see the Storybook interface

3. **Browse Components**
   - **Financial/FinancialDashboard** - Main dashboard with multiple scenarios
   - **Financial/FinancialStatementViewer** - Statement viewer component

4. **Interactive Testing**
   - Use **Controls** panel to modify props (companyId, userType, etc.)
   - Use **Viewport** addon to test different screen sizes
   - Switch between stories to see different scenarios

5. **Story Scenarios Available**
   - **Default** - Basic functionality
   - **BeginnerUser** - Simplified interface for new users
   - **AdvancedUser** - Full feature set
   - **ExpertUser** - Professional tools
   - **MobileView/TabletView** - Responsive design testing
   - **LoadingState** - Loading behavior
   - **ErrorState** - Error handling

### Development
```bash
# Start Storybook development server
npm run storybook

# Build Storybook for production
npm run build-storybook
```

### Testing
```bash
# Run Storybook tests (requires Storybook to be running)
npm run test-storybook

# Run with UI mode
npm run test-storybook:ui

# Run in headed mode (visible browser)
npm run test-storybook:headed
```

### CI Integration
Storybook tests run in CI parallel to frontend integration tests:
- **Trigger**: After `frontend-tests` job completes
- **Parallel**: Runs alongside `frontend-integration-tests`
- **Dependencies**: `frontend-tests` must pass first
- **Artifacts**: Test results uploaded for debugging

## Configuration Details

### Vite Integration
The Storybook configuration includes Vite customization to handle:
- Node.js module externalization (`fs`, `path`)
- Global definitions for browser compatibility
- Alias resolution for clean imports

### MSW Setup
- **Browser Compatibility**: Node.js imports handled via Vite externalization
- **Mock Scenarios**: Reuses existing test infrastructure
- **GraphQL Responses**: Loads from existing test data files
- **Error Handling**: Graceful fallbacks for missing files

### Test Runner
- **Playwright Integration**: Uses Playwright for browser automation
- **Story Coverage**: Tests all stories automatically
- **Visual Testing**: Screenshot comparison capabilities
- **CI Integration**: Runs in GitHub Actions workflow

## Best Practices

### Story Creation
1. **Reuse Existing Mocks**: Use established MSW scenarios
2. **Component Isolation**: Each story should be self-contained
3. **Error Boundaries**: Wrap components in ErrorBoundary
4. **Provider Setup**: Include all necessary providers
5. **Scenario Coverage**: Test different states and user types

### MSW Integration
1. **Don't Simplify**: MSW is core to testing philosophy
2. **Use Existing Scenarios**: Leverage established mock data
3. **Handle Node.js Imports**: Configure Vite for browser compatibility
4. **Maintain Consistency**: Keep same patterns as regular tests

### Testing Strategy
1. **Visual Testing**: Use Storybook for component-level testing
2. **Integration Testing**: Combine with existing test suite
3. **CI Integration**: Run in parallel with other tests
4. **Error Handling**: Test error states and boundaries

## Troubleshooting

### Common Issues

1. **Node.js Import Errors**
   - **Cause**: `fs` and `path` modules in browser environment
   - **Solution**: Vite externalization configuration in `main.tsx`

2. **JSX Parsing Errors**
   - **Cause**: TypeScript files with JSX content
   - **Solution**: Use `.tsx` extension for files with JSX

3. **MSW Integration Issues**
   - **Cause**: Browser compatibility with Node.js modules
   - **Solution**: Proper Vite configuration for externalization

4. **Test Runner Not Found**
   - **Cause**: Missing `@storybook/test-runner` package
   - **Solution**: Install with `npm install --save-dev @storybook/test-runner`

### Debug Steps
1. Check file extensions (`.tsx` for JSX files)
2. Verify MSW setup and mock scenarios
3. Ensure all providers are properly configured
4. Test with `npm run build-storybook` first
5. Check CI logs for specific error messages

## Future Enhancements

### Planned Features
- **Visual Regression Testing**: Screenshot comparison
- **Accessibility Testing**: Automated a11y checks
- **Performance Testing**: Component performance metrics
- **Cross-Browser Testing**: Multiple browser support

### Integration Improvements
- **E2E Test Integration**: Connect with Playwright E2E tests
- **Component Documentation**: Auto-generated docs
- **Design System**: Component library organization
- **Testing Utilities**: Shared testing helpers

## Related Documentation

- [Frontend Testing Strategy](../testing/frontend-testing-strategy.md)
- [MSW Integration Guide](../testing/msw-integration.md)
- [CI/CD Pipeline](../ci/ci-pipeline.md)
- [Component Architecture](../technical/component-architecture.md)