# Vitest Migration Plan with MSW Integration

**Date:** January 15, 2025  
**Branch:** `frontend/vitest-migration`  
**Status:** Planning Phase  
**Author:** Frontend Development Team  

## Overview

This document outlines the staged migration from Jest to Vitest with MSW (Mock Service Worker) integration for the EconGraph frontend. The migration will improve test performance, provide better GraphQL mocking with schema validation, and prepare the foundation for Storybook visual testing integration.

## Current State Analysis

### Test Infrastructure
- **Total Test Files:** 25
- **Testing Framework:** Jest + React Testing Library
- **Mocking:** Jest mocks with D3.js workarounds
- **MSW Setup:** Existing in `frontend/src/test-utils/mocks/`
- **GraphQL:** Custom mocking with `jest.mock()`

### Test File Distribution
- **Financial Components** (8 tests): Dashboard, StatementViewer, TrendAnalysis, etc.
- **Chart Components** (5 tests): InteractiveChart, ProfessionalChart, Collaboration
- **Pages** (3 tests): Dashboard, SeriesExplorer, DataSources
- **Auth Components** (2 tests): LoginDialog, UserProfile
- **Integration Tests** (2 tests): XBRL financial, E2E workflows
- **Contexts** (1 test): ThemeContext
- **Hooks** (1 test): useSeriesData

### Current Jest Configuration Issues
```json
{
  "transformIgnorePatterns": [
    "node_modules/(?!(@apollo/client|d3-geo|d3-zoom|d3-scale|d3-scale-chromatic|d3-array|d3-selection)/)"
  ]
}
```

## Migration Goals

### Primary Objectives
1. **Performance Improvement:** Target 50% faster test execution
2. **Better D3.js Integration:** Native ES module support
3. **Schema Validation:** GraphQL queries validate against schema
4. **Maintainable Mocks:** Centralized, verifiable mock data
5. **Storybook Preparation:** Foundation for visual testing

### Success Metrics
- Test execution time: 50% improvement
- Test reliability: No flaky tests
- Developer experience: Faster feedback
- CI/CD performance: Reduced build times
- Mock maintainability: Schema-validated GraphQL mocks

## Staged Migration Plan

### Stage 1: Foundation Setup + MSW Integration
**Duration:** 2-3 days  
**Status:** In Progress  

#### Goals
- Install Vitest alongside Jest
- Integrate MSW with Vitest
- Create GraphQL schema validation
- Set up test environment compatibility

#### Tasks

1. **Install Dependencies**
   ```bash
   npm install --save-dev vitest @vitest/ui @vitest/browser
   npm install --save-dev @vitejs/plugin-react jsdom
   npm install --save-dev msw @graphql-tools/mock
   ```

2. **Create Vitest Configuration**
   ```typescript
   // vitest.config.ts
   import { defineConfig } from 'vitest/config'
   import react from '@vitejs/plugin-react'
   import path from 'path'

   export default defineConfig({
     plugins: [react()],
     test: {
       environment: 'jsdom',
       setupFiles: ['./src/setupTests.vitest.ts'],
       globals: true,
       deps: {
         inline: ['d3', 'd3-geo', 'd3-zoom', 'd3-scale', 'd3-scale-chromatic', 'd3-array', 'd3-selection']
       }
     },
     resolve: {
       alias: {
         '@': path.resolve(__dirname, './src')
       }
     }
   })
   ```

3. **Create Vitest Setup File**
   ```typescript
   // src/setupTests.vitest.ts
   import '@testing-library/jest-dom'
   import { beforeAll, afterEach, afterAll } from 'vitest'
   import { server } from './test-utils/mocks/server'

   // Start MSW server before all tests
   beforeAll(() => server.listen({ onUnhandledRequest: 'error' }))

   // Reset handlers after each test
   afterEach(() => server.resetHandlers())

   // Clean up after all tests
   afterAll(() => server.close())
   ```

4. **Enhance MSW with Schema Validation**
   ```typescript
   // src/test-utils/mocks/schema.ts
   import { buildSchema } from 'graphql'
   import { addMocksToSchema } from '@graphql-tools/mock'

   const schema = buildSchema(`
     type Query {
       series(id: ID!): Series
       seriesData(seriesId: ID!): SeriesData
       searchSeries(query: String!): SearchResults
       dataSources: [DataSource]
       crawlerStatus: CrawlerStatus
     }
     
     type Series {
       id: ID!
       title: String!
       description: String
       sourceId: String
       frequency: String
       units: String
     }
     
     # ... rest of schema
   `)

   export const mockedSchema = addMocksToSchema({ schema })
   ```

5. **Create GraphQL Mock Files**
   ```typescript
   // src/test-utils/mocks/graphql/financial-queries.ts
   export const GET_FINANCIAL_DASHBOARD = `
     query GetFinancialDashboard($companyId: ID!) {
       company(id: $companyId) {
         id
         name
         ticker
         financialStatements {
           id
           type
           period
           data
         }
       }
     }
   `

   export const GET_FINANCIAL_STATEMENT = `
     query GetFinancialStatement($statementId: ID!) {
       financialStatement(id: $statementId) {
         id
         type
         period
         lineItems {
           id
           name
           value
           category
         }
       }
     }
   `
   ```

#### Success Criteria
- [ ] Vitest runs with MSW integration
- [ ] GraphQL schema validation works
- [ ] Mock files are verifiable against schema
- [ ] Jest tests still pass
- [ ] No performance regressions

---

### Stage 2: Financial Components Migration + MSW
**Duration:** 3-4 days  
**Status:** Pending  

#### Target Files
- `components/financial/__tests__/FinancialDashboard.test.tsx`
- `components/financial/__tests__/FinancialStatementViewer.test.tsx`
- `components/financial/__tests__/TrendAnalysisChart.test.tsx`
- `components/financial/__tests__/PeerComparisonChart.test.tsx`
- `components/financial/__tests__/BenchmarkComparison.test.tsx`
- `components/financial/__tests__/FinancialAlerts.test.tsx`
- `components/financial/__tests__/FinancialMobile.test.tsx`
- `components/financial/__tests__/FinancialExport.test.tsx`

#### Tasks

1. **Create Financial GraphQL Mocks**
   ```typescript
   // src/test-utils/mocks/graphql/financial-mocks.ts
   import { graphql, HttpResponse } from 'msw'
   import { GET_FINANCIAL_DASHBOARD, GET_FINANCIAL_STATEMENT } from './financial-queries'

   export const financialHandlers = [
     graphql.query('GetFinancialDashboard', ({ variables }) => {
       const { companyId } = variables as { companyId: string }
       
       return HttpResponse.json({
         data: {
           company: {
             id: companyId,
             name: 'Apple Inc.',
             ticker: 'AAPL',
             financialStatements: [
               {
                 id: 'statement-1',
                 type: 'INCOME_STATEMENT',
                 period: '2024-Q3',
                 data: { /* mock financial data */ }
               }
             ]
           }
         }
       })
     }),

     graphql.query('GetFinancialStatement', ({ variables }) => {
       const { statementId } = variables as { statementId: string }
       
       return HttpResponse.json({
         data: {
           financialStatement: {
             id: statementId,
             type: 'INCOME_STATEMENT',
             period: '2024-Q3',
             lineItems: [
               {
                 id: 'revenue',
                 name: 'Total Revenue',
                 value: 89498000000,
                 category: 'REVENUE'
               }
             ]
           }
         }
       })
     })
   ]
   ```

2. **Migrate Financial Component Tests**
   ```typescript
   // components/financial/__tests__/FinancialDashboard.test.tsx
   import { render, screen, waitFor } from '@testing-library/react'
   import { server } from '../../../test-utils/mocks/server'
   import { financialHandlers } from '../../../test-utils/mocks/graphql/financial-mocks'
   import { FinancialDashboard } from '../FinancialDashboard'

   describe('FinancialDashboard', () => {
     beforeEach(() => {
       server.use(...financialHandlers)
     })

     it('should render financial dashboard with company data', async () => {
       render(<FinancialDashboard companyId="0000320193" />)
       
       await waitFor(() => {
         expect(screen.getByText('Apple Inc.')).toBeInTheDocument()
       })
     })
   })
   ```

3. **Add Schema Validation for Financial Queries**
   ```typescript
   // src/test-utils/mocks/validate-schema.ts
   import { validate } from 'graphql'
   import { parse } from 'graphql'
   import { GET_FINANCIAL_DASHBOARD } from './graphql/financial-queries'

   export function validateFinancialQueries() {
     const query = parse(GET_FINANCIAL_DASHBOARD)
     const errors = validate(schema, query)
     
     if (errors.length > 0) {
       throw new Error(`Schema validation failed: ${errors.map(e => e.message).join(', ')}`)
     }
   }
   ```

#### Success Criteria
- [ ] Financial components work with MSW
- [ ] GraphQL queries validate against schema
- [ ] Mock data is realistic and verifiable
- [ ] No fragile mocks
- [ ] Material-UI components work correctly
- [ ] Chart.js integration works

---

### Stage 3: Global Analysis Components Migration + MSW
**Duration:** 3-4 days  
**Status:** Pending  

#### Target Files
- Global analysis components (when they have tests)
- D3.js integration tests

#### Tasks

1. **Create Global Analysis GraphQL Mocks**
   ```typescript
   // src/test-utils/mocks/graphql/global-analysis-mocks.ts
   export const globalAnalysisHandlers = [
     graphql.query('GetCountryData', ({ variables }) => {
       const { countryCodes } = variables as { countryCodes: string[] }
       
       return HttpResponse.json({
         data: {
           countries: countryCodes.map(code => ({
             code,
             name: getCountryName(code),
             indicators: {
               gdp: Math.random() * 1000000,
               population: Math.random() * 100000000,
               inflation: Math.random() * 10
             }
           }))
         }
       })
     }),

     graphql.query('GetGlobalEvents', ({ variables }) => {
       return HttpResponse.json({
         data: {
           globalEvents: [
             {
               id: 'event-1',
               title: 'Economic Summit',
               date: '2024-01-15',
               countries: ['US', 'CN', 'EU'],
               impact: 'HIGH'
             }
           ]
         }
       })
     })
   ]
   ```

2. **Migrate D3.js Components with MSW**
   ```typescript
   // components/global/__tests__/InteractiveWorldMap.test.tsx
   import { render, screen, waitFor } from '@testing-library/react'
   import { server } from '../../../test-utils/mocks/server'
   import { globalAnalysisHandlers } from '../../../test-utils/mocks/graphql/global-analysis-mocks'
   import { InteractiveWorldMap } from '../InteractiveWorldMap'

   describe('InteractiveWorldMap', () => {
     beforeEach(() => {
       server.use(...globalAnalysisHandlers)
     })

     it('should render world map with country data', async () => {
       render(<InteractiveWorldMap selectedCountries={['US', 'CN']} />)
       
       await waitFor(() => {
         expect(screen.getByTestId('world-map')).toBeInTheDocument()
       })
     })
   })
   ```

#### Success Criteria
- [ ] Global analysis components work with MSW
- [ ] D3.js integration works with mocked data
- [ ] Country data is realistic and verifiable
- [ ] World map interactions work
- [ ] No memory leaks in D3.js tests

---

### Stage 4: Chart Components Migration + MSW
**Duration:** 3-4 days  
**Status:** Pending  

#### Target Files
- `components/charts/__tests__/InteractiveChart.test.tsx`
- `components/charts/__tests__/InteractiveChartWithCollaboration.test.tsx`
- `components/charts/__tests__/ChartCollaboration.test.tsx`
- `components/charts/__tests__/ChartCollaborationConnected.test.tsx`
- `components/charts/__tests__/ChartCollaborationConnected.integration.test.tsx`
- `components/charts/__tests__/ProfessionalChart.test.tsx`

#### Tasks

1. **Create Chart GraphQL Mocks**
   ```typescript
   // src/test-utils/mocks/graphql/chart-mocks.ts
   export const chartHandlers = [
     graphql.query('GetChartData', ({ variables }) => {
       const { seriesId, timeRange } = variables as { seriesId: string, timeRange: string }
       
       return HttpResponse.json({
         data: {
           chartData: {
             seriesId,
             timeRange,
             dataPoints: generateMockDataPoints(timeRange),
             metadata: {
               title: 'Economic Indicator',
               units: 'Billions USD',
               frequency: 'Monthly'
             }
           }
         }
       })
     })
   ]
   ```

2. **Migrate Chart Components with MSW**
   ```typescript
   // components/charts/__tests__/InteractiveChart.test.tsx
   import { render, screen, waitFor } from '@testing-library/react'
   import { server } from '../../../test-utils/mocks/server'
   import { chartHandlers } from '../../../test-utils/mocks/graphql/chart-mocks'
   import { InteractiveChart } from '../InteractiveChart'

   describe('InteractiveChart', () => {
     beforeEach(() => {
       server.use(...chartHandlers)
     })

     it('should render chart with data', async () => {
       render(<InteractiveChart seriesId="gdp-series-1" timeRange="1Y" />)
       
       await waitFor(() => {
         expect(screen.getByTestId('chart-container')).toBeInTheDocument()
       })
     })
   })
   ```

#### Success Criteria
- [ ] Chart components work with MSW
- [ ] Chart.js integration works with mocked data
- [ ] Data visualization is realistic
- [ ] Collaboration features function properly
- [ ] Performance is maintained

---

### Stage 5: Layout and Auth Components Migration + MSW
**Duration:** 2-3 days  
**Status:** Pending  

#### Target Files
- `components/auth/__tests__/LoginDialog.test.tsx`
- `components/auth/__tests__/UserProfile.test.tsx`
- `pages/__tests__/Dashboard.test.tsx`
- `pages/__tests__/SeriesExplorer.test.tsx`
- `pages/__tests__/DataSources.test.tsx`
- `contexts/__tests__/ThemeContext.test.tsx`
- `hooks/__tests__/useSeriesData.test.tsx`

#### Tasks

1. **Create Auth GraphQL Mocks**
   ```typescript
   // src/test-utils/mocks/graphql/auth-mocks.ts
   export const authHandlers = [
     graphql.query('GetCurrentUser', () => {
       return HttpResponse.json({
         data: {
           currentUser: {
             id: 'user-1',
             name: 'Test User',
             email: 'test@example.com',
             role: 'USER'
           }
         }
       })
     }),

     graphql.mutation('Login', ({ variables }) => {
       const { email, password } = variables as { email: string, password: string }
       
       if (email === 'test@example.com' && password === 'password') {
         return HttpResponse.json({
           data: {
             login: {
               user: {
                 id: 'user-1',
                 name: 'Test User',
                 email: 'test@example.com'
               },
               token: 'mock-jwt-token'
             }
           }
         })
       }
       
       return HttpResponse.json({
         data: null,
         errors: [{ message: 'Invalid credentials' }]
       })
     })
   ]
   ```

2. **Migrate Auth Components with MSW**
   ```typescript
   // components/auth/__tests__/LoginDialog.test.tsx
   import { render, screen, waitFor } from '@testing-library/react'
   import userEvent from '@testing-library/user-event'
   import { server } from '../../../test-utils/mocks/server'
   import { authHandlers } from '../../../test-utils/mocks/graphql/auth-mocks'
   import { LoginDialog } from '../LoginDialog'

   describe('LoginDialog', () => {
     beforeEach(() => {
       server.use(...authHandlers)
     })

     it('should login successfully with valid credentials', async () => {
       const user = userEvent.setup()
       render(<LoginDialog open={true} onClose={() => {}} />)
       
       await user.type(screen.getByLabelText('Email'), 'test@example.com')
       await user.type(screen.getByLabelText('Password'), 'password')
       await user.click(screen.getByRole('button', { name: 'Login' }))
       
       await waitFor(() => {
         expect(screen.getByText('Login successful')).toBeInTheDocument()
       })
     })
   })
   ```

#### Success Criteria
- [ ] Auth components work with MSW
- [ ] User management works with mocked data
- [ ] Login/logout flows work correctly
- [ ] Page components work correctly
- [ ] Context and hooks function properly
- [ ] Navigation tests pass

---

### Stage 6: Integration Tests and Cleanup + MSW
**Duration:** 3-4 days  
**Status:** Pending  

#### Target Files
- `__tests__/integration/xbrl-financial-integration.test.tsx`
- `__tests__/e2e-user-workflows.test.tsx`
- `__tests__/e2e-integration.test.tsx`

#### Tasks

1. **Create Comprehensive MSW Setup**
   ```typescript
   // src/test-utils/mocks/comprehensive-server.ts
   import { setupServer } from 'msw/node'
   import { financialHandlers } from './graphql/financial-mocks'
   import { globalAnalysisHandlers } from './graphql/global-analysis-mocks'
   import { chartHandlers } from './graphql/chart-mocks'
   import { authHandlers } from './graphql/auth-mocks'

   export const comprehensiveServer = setupServer(
     ...financialHandlers,
     ...globalAnalysisHandlers,
     ...chartHandlers,
     ...authHandlers
   )
   ```

2. **Migrate Integration Tests with MSW**
   ```typescript
   // __tests__/integration/xbrl-financial-integration.test.tsx
   import { render, screen, waitFor } from '@testing-library/react'
   import { comprehensiveServer } from '../test-utils/mocks/comprehensive-server'
   import { FinancialDashboard } from '../../components/financial/FinancialDashboard'

   describe('XBRL Financial Integration', () => {
     beforeEach(() => {
       comprehensiveServer.use(...financialHandlers)
     })

     it('should complete financial analysis workflow', async () => {
       render(<FinancialDashboard companyId="0000320193" />)
       
       await waitFor(() => {
         expect(screen.getByText('Apple Inc.')).toBeInTheDocument()
       })
       
       // Test financial statement interaction
       await user.click(screen.getByText('Income Statement'))
       
       await waitFor(() => {
         expect(screen.getByText('Total Revenue')).toBeInTheDocument()
       })
     })
   })
   ```

3. **Remove Jest and Finalize Vitest + MSW**
   ```json
   // package.json scripts
   {
     "scripts": {
       "test": "vitest",
       "test:ui": "vitest --ui",
       "test:run": "vitest run",
       "test:coverage": "vitest run --coverage"
     }
   }
   ```

#### Success Criteria
- [ ] All tests pass with Vitest + MSW
- [ ] GraphQL schema validation works
- [ ] Mock data is verifiable and realistic
- [ ] Jest is completely removed
- [ ] CI/CD works with Vitest + MSW
- [ ] Performance is improved

---

## MSW Integration Benefits

### 1. Schema Validation
- GraphQL queries validate against schema
- Catch breaking changes early
- Ensure mock data matches real API

### 2. Realistic Testing
- Mock data mirrors production
- Test real GraphQL queries
- Verify component behavior with real data shapes

### 3. Maintainable Mocks
- Centralized mock definitions
- Easy to update when schema changes
- No fragile component-level mocks

### 4. Better Developer Experience
- Fast feedback with realistic data
- Easy to debug GraphQL issues
- Consistent testing patterns

## Risk Mitigation

### Parallel Development Approach
- Keep Jest running during migration
- Migrate tests incrementally
- Validate each stage before proceeding
- Maintain CI/CD throughout

### Backup Strategy
- Backup current test setup
- Test each stage thoroughly
- Keep Jest as fallback until Stage 6
- Document any breaking changes

### Rollback Plan
- Each stage is independently reversible
- Jest configuration preserved until Stage 6
- MSW setup can be disabled if needed
- Clear documentation of changes

## Timeline Estimate

| Stage | Duration | Status | Dependencies |
|-------|----------|--------|--------------|
| Stage 1: Foundation + MSW | 2-3 days | In Progress | None |
| Stage 2: Financial Components | 3-4 days | Pending | Stage 1 |
| Stage 3: Global Analysis | 3-4 days | Pending | Stage 1 |
| Stage 4: Chart Components | 3-4 days | Pending | Stage 1 |
| Stage 5: Layout/Auth | 2-3 days | Pending | Stage 1 |
| Stage 6: Integration/Cleanup | 3-4 days | Pending | Stages 2-5 |

**Total Duration:** 15-20 days

## Next Steps

1. **Complete Stage 1** (Foundation + MSW Setup)
2. **Create GraphQL schema files** for validation
3. **Begin Stage 2** (Financial Components Migration)
4. **Monitor performance improvements**
5. **Document lessons learned**

## Future Considerations

### Storybook Integration
- Vitest + MSW foundation enables Storybook testing
- Visual regression testing with Chromatic
- Component playground for client demos

### CI/CD Optimization
- Parallel test execution
- Cached dependencies
- Optimized test reporting

### Performance Monitoring
- Track test execution times
- Monitor memory usage
- Identify performance bottlenecks

---

**Document Version:** 1.0  
**Last Updated:** January 15, 2025  
**Next Review:** After Stage 1 completion
