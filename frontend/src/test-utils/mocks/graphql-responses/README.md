# GraphQL Response Mocks

This directory contains organized GraphQL response mocks for comprehensive
testing. Each API call has multiple response scenarios to test different states
and edge cases.

## Directory Structure

Each GraphQL operation has its own directory named using snake_case (matching
GraphQL conventions):

- `get_financial_dashboard/` - Company financial dashboard data
- `get_financial_statement/` - Individual financial statement data
- `get_financial_ratios/` - Financial ratio calculations
- `get_ratio_benchmarks/` - Industry benchmark data
- `get_ratio_explanation/` - Ratio explanation and educational content

## Response Types

Each operation directory contains multiple response files:

### Success Responses

- `success.json` - Complete, valid response with all data
- `partial_data.json` - Response with some missing or incomplete data
- `loading.json` - Response indicating data is still being processed

### Error Responses

- `not_found.json` - Resource not found (404 equivalent)
- `error.json` - Server error or processing failure
- `empty.json` - Valid response with empty data (when applicable)

## Usage in Tests

These JSON files are loaded by MSW handlers to provide realistic API responses
for different test scenarios:

```typescript
// Example: Test loading state
const response = await loadMockResponse(
  'get_financial_dashboard',
  'loading.json'
);

// Example: Test error handling
const response = await loadMockResponse('get_financial_ratios', 'error.json');

// Example: Test success case
const response = await loadMockResponse(
  'get_financial_statement',
  'success.json'
);
```

## Benefits

1. **Comprehensive Testing** - Test all possible API response states
2. **Realistic Data** - Use actual GraphQL response structures
3. **Easy Maintenance** - Update mock data without changing test code
4. **Developer Friendly** - Easy to inspect and verify mock responses
5. **Schema Validation** - Responses match actual GraphQL schema

## Adding New Responses

1. Create new JSON file in appropriate operation directory
2. Follow existing naming convention (success, error, not_found, etc.)
3. Ensure response structure matches GraphQL schema
4. Update MSW handlers to use new response file
5. Add tests for new response scenario
