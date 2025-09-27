# MSW (Mock Service Worker) Setup

This directory contains the MSW setup for intercepting GraphQL requests during testing.

## Debug Output

MSW debug output is disabled by default to keep test output clean. To enable debug output when needed:

### Option 1: Environment Variable

```bash
MSW_DEBUG=true npm test
```

### Option 2: In your test file

```typescript
// Temporarily enable debug for specific tests
process.env.MSW_DEBUG = "true";
```

### Option 3: For debugging specific operations

```typescript
// Enable debug for a specific test
beforeAll(() => {
  process.env.MSW_DEBUG = "true";
});

afterAll(() => {
  delete process.env.MSW_DEBUG;
});
```

## Debug Output Includes

- GraphQL request interception logs
- Operation names and variables
- Current scenario (success/error/loading)
- Response data
- Error handling

## Files

- `server.ts` - Main MSW server setup with full GraphQL support
- `simpleServer.ts` - Simplified server for environments with import issues
- `README.md` - This documentation file

## Usage

The MSW server is automatically started in `setupTests.ts` and provides mock responses for all GraphQL operations used in the admin frontend.
